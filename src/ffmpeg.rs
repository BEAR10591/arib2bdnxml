//! FFmpeg wrapper: open file, detect ARIB subtitle stream, init decoder, composite AVSubtitle to RGBA.

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_int;
use std::ptr;

use crate::bitmap::BitmapData;
use crate::config;
use crate::ffmpeg_sys::*;

const AV_NOPTS_VALUE: i64 = i64::MIN;
const INVALID_DISPLAY_TIME: u32 = 0xFFFF_FFFF;

/// Video stream info (resolution, FPS, start time).
#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub width: i32,
    pub height: i32,
    pub fps: f64,
    pub start_time: f64,
}

/// A single subtitle frame (bitmap or clear command).
#[derive(Debug)]
#[allow(dead_code)] // pts used internally for timestamp calculation
pub struct SubtitleFrame {
    pub bitmap: Option<BitmapData>,
    pub pts: i64,
    pub timestamp: f64,
    pub start_time: f64,
    pub end_time: f64,
    pub x: i32,
    pub y: i32,
}

pub struct FfmpegWrapper {
    debug: bool,
    format_ctx: *mut AVFormatContext,
    codec_ctx: *mut AVCodecContext,
    codec: *const AVCodec,
    subtitle_stream_index: c_int,
    video_stream_index: c_int,
    video_info: VideoInfo,
}

unsafe impl Send for FfmpegWrapper {}

fn pts_to_seconds(pts: i64, time_base: AVRational) -> f64 {
    if pts == AV_NOPTS_VALUE {
        return 0.0;
    }
    let num = time_base.num as f64;
    let den = time_base.den as f64;
    if den == 0.0 {
        return 0.0;
    }
    pts as f64 * (num / den)
}

fn ffmpeg_strerror(err: c_int) -> String {
    let mut buf = [0i8; 64];
    unsafe {
        av_strerror(err, buf.as_mut_ptr(), buf.len());
        CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
    }
}

fn codec_name_has_arib(name: *const std::ffi::c_char) -> bool {
    if name.is_null() {
        return false;
    }
    let s = unsafe { CStr::from_ptr(name).to_string_lossy() };
    s.contains("arib") || s.contains("libaribcaption")
}

/// Probes a file for video stream resolution. Returns (width, height) or error if no video stream.
/// Used for .mks companion .mkv resolution when --anamorphic is set.
pub fn probe_video_resolution(filename: &str) -> anyhow::Result<(i32, i32)> {
    let c_path = CString::new(filename).map_err(|e| anyhow::anyhow!("path: {}", e))?;
    unsafe {
        let mut format_opts: *mut AVDictionary = ptr::null_mut();
        let k1 = CString::new("analyzeduration").unwrap();
        let v1 = CString::new("5000000").unwrap();
        av_dict_set(&mut format_opts, k1.as_ptr(), v1.as_ptr(), 0);
        let k2 = CString::new("probesize").unwrap();
        let v2 = CString::new("5000000").unwrap();
        av_dict_set(&mut format_opts, k2.as_ptr(), v2.as_ptr(), 0);

        let mut ctx: *mut AVFormatContext = ptr::null_mut();
        let ret = avformat_open_input(
            &mut ctx,
            c_path.as_ptr(),
            ptr::null(),
            &mut format_opts,
        );
        if !format_opts.is_null() {
            av_dict_free(&mut format_opts);
        }
        if ret < 0 {
            anyhow::bail!(
                "Failed to open file: {} ({})",
                filename,
                ffmpeg_strerror(ret)
            );
        }

        let ret = avformat_find_stream_info(ctx, ptr::null_mut());
        if ret < 0 {
            avformat_close_input(&mut ctx);
            anyhow::bail!("Failed to get stream info: {}", ffmpeg_strerror(ret));
        }

        let nb_streams = (*ctx).nb_streams;
        let mut width = 0i32;
        let mut height = 0i32;
        for i in 0..nb_streams {
            let stream = *(*ctx).streams.add(i as usize);
            if stream.is_null() {
                continue;
            }
            let codecpar = (*stream).codecpar;
            if codecpar.is_null() {
                continue;
            }
            if (*codecpar).codec_type == AVMediaType_AVMEDIA_TYPE_VIDEO {
                width = (*codecpar).width;
                height = (*codecpar).height;
                break;
            }
        }

        avformat_close_input(&mut ctx);

        if width <= 0 || height <= 0 {
            anyhow::bail!("No video stream found in {}", filename);
        }
        Ok((width, height))
    }
}

impl FfmpegWrapper {
    pub fn new() -> Self {
        unsafe {
            av_log_set_level(AV_LOG_FATAL as c_int);
        }
        FfmpegWrapper {
            debug: false,
            format_ctx: ptr::null_mut(),
            codec_ctx: ptr::null_mut(),
            codec: ptr::null(),
            subtitle_stream_index: -1,
            video_stream_index: -1,
            video_info: VideoInfo {
                width: 0,
                height: 0,
                fps: 0.0,
                start_time: 0.0,
            },
        }
    }

    pub fn set_debug(&mut self, debug: bool) {
        self.debug = debug;
        unsafe {
            av_log_set_level(if debug {
                AV_LOG_INFO as c_int
            } else {
                AV_LOG_FATAL as c_int
            });
        }
    }

    pub fn open_file(&mut self, filename: &str) -> anyhow::Result<()> {
        let c_path = CString::new(filename).map_err(|e| anyhow::anyhow!("path: {}", e))?;

        let mut format_opts: *mut AVDictionary = ptr::null_mut();
        unsafe {
            let k1 = CString::new("analyzeduration").unwrap();
            let v1 = CString::new("150000000").unwrap();
            av_dict_set(&mut format_opts, k1.as_ptr(), v1.as_ptr(), 0);
            let k2 = CString::new("probesize").unwrap();
            let v2 = CString::new("150000000").unwrap();
            av_dict_set(&mut format_opts, k2.as_ptr(), v2.as_ptr(), 0);
            let k3 = CString::new("fflags").unwrap();
            let v3 = CString::new("+genpts+igndts").unwrap();
            av_dict_set(&mut format_opts, k3.as_ptr(), v3.as_ptr(), 0);

            let mut ctx: *mut AVFormatContext = ptr::null_mut();
            let ret = avformat_open_input(
                &mut ctx,
                c_path.as_ptr(),
                ptr::null(),
                &mut format_opts,
            );
            if !format_opts.is_null() {
                av_dict_free(&mut format_opts);
            }
            if ret < 0 {
                anyhow::bail!(
                    "Failed to open file: {} ({})",
                    filename,
                    ffmpeg_strerror(ret)
                );
            }
            self.format_ctx = ctx;
        }

        unsafe {
            let ret = avformat_find_stream_info(self.format_ctx, ptr::null_mut());
            if ret < 0 {
                self.close();
                anyhow::bail!("Failed to get stream info: {}", ffmpeg_strerror(ret));
            }

            let nb_streams = (*self.format_ctx).nb_streams;
            if self.debug {
                eprintln!("Searching for subtitle stream... (total streams: {})", nb_streams);
            }

            for i in 0..nb_streams {
                let stream = *(*self.format_ctx).streams.add(i as usize);
                if stream.is_null() {
                    continue;
                }
                let codecpar = (*stream).codecpar;
                if codecpar.is_null() {
                    continue;
                }
                if (*codecpar).codec_type == AVMediaType_AVMEDIA_TYPE_SUBTITLE {
                    let codec = avcodec_find_decoder((*codecpar).codec_id);
                    if !codec.is_null() && codec_name_has_arib((*codec).name) {
                        self.subtitle_stream_index = i as c_int;
                        if self.debug {
                            eprintln!("Subtitle stream found: index {}", i);
                        }
                        break;
                    }
                }
            }

            if self.subtitle_stream_index < 0 {
                self.close();
                anyhow::bail!("ARIB subtitle stream not found.");
            }

            for i in 0..nb_streams {
                let stream = *(*self.format_ctx).streams.add(i as usize);
                if !stream.is_null()
                    && !(*stream).codecpar.is_null()
                    && (*(*stream).codecpar).codec_type == AVMediaType_AVMEDIA_TYPE_VIDEO
                {
                    self.video_stream_index = i as c_int;
                    break;
                }
            }

            if self.video_stream_index >= 0 {
                let stream = *(*self.format_ctx)
                    .streams
                    .add(self.video_stream_index as usize);
                let par = (*stream).codecpar;
                self.video_info.width = (*par).width;
                self.video_info.height = (*par).height;
                let avg = (*stream).avg_frame_rate;
                let r = (*stream).r_frame_rate;
                if avg.num > 0 && avg.den > 0 {
                    self.video_info.fps =
                        (avg.num as f64) / (avg.den as f64);
                } else if r.num > 0 && r.den > 0 {
                    self.video_info.fps = (r.num as f64) / (r.den as f64);
                }
            }

            let start = (*self.format_ctx).start_time;
            self.video_info.start_time = if start != AV_NOPTS_VALUE {
                start as f64 / AV_TIME_BASE as f64
            } else {
                0.0
            };
        }

        Ok(())
    }

    pub fn get_video_info(&self) -> VideoInfo {
        self.video_info.clone()
    }

    pub fn init_decoder(
        &mut self,
        libaribcaption_opts: &HashMap<String, String>,
    ) -> anyhow::Result<()> {
        if self.subtitle_stream_index < 0 {
            anyhow::bail!("Subtitle stream not configured.");
        }

        unsafe {
            let stream = *(*self.format_ctx)
                .streams
                .add(self.subtitle_stream_index as usize);
            self.codec = avcodec_find_decoder((*stream).codecpar.as_ref().unwrap().codec_id);
            if self.codec.is_null() {
                anyhow::bail!("Decoder not found.");
            }

            self.codec_ctx = avcodec_alloc_context3(self.codec);
            if self.codec_ctx.is_null() {
                anyhow::bail!("Failed to create decoder context.");
            }

            let ret = avcodec_parameters_to_context(
                self.codec_ctx,
                (*stream).codecpar,
            );
            if ret < 0 {
                avcodec_free_context(&mut self.codec_ctx);
                anyhow::bail!("Failed to copy decoder parameters.");
            }

            (*self.codec_ctx).time_base = (*stream).time_base;

            let mut opts_dict: *mut AVDictionary = ptr::null_mut();
            if codec_name_has_arib((*self.codec).name) {
                let k_st = CString::new("sub_type").unwrap();
                let v_st = CString::new("bitmap").unwrap();
                av_dict_set(&mut opts_dict, k_st.as_ptr(), v_st.as_ptr(), 0);
                let canvas_size = match libaribcaption_opts.get("canvas_size") {
                    Some(s) => s.as_str(),
                    None => anyhow::bail!("canvas_size not set."),
                };
                let c_canvas = CString::new(canvas_size).unwrap();
                let k_canvas = CString::new("canvas_size").unwrap();
                av_dict_set(&mut opts_dict, k_canvas.as_ptr(), c_canvas.as_ptr(), 0);
                if let Ok((w, h)) = config::parse_canvas_size(canvas_size) {
                    (*self.codec_ctx).width = w;
                    (*self.codec_ctx).height = h;
                }
                if                 (*self.codec_ctx).pix_fmt == AVPixelFormat_AV_PIX_FMT_NONE
                    || (*self.codec_ctx).pix_fmt == -1
                {
                    (*self.codec_ctx).pix_fmt = AVPixelFormat_AV_PIX_FMT_RGBA;
                }
            }

            for (k, v) in libaribcaption_opts {
                if k == "sub_type" || k == "canvas_size" {
                    continue;
                }
                let ck = CString::new(k.as_str()).unwrap();
                let cv = CString::new(v.as_str()).unwrap();
                av_dict_set(&mut opts_dict, ck.as_ptr(), cv.as_ptr(), 0);
            }

            let ret = avcodec_open2(
                self.codec_ctx,
                self.codec,
                &mut opts_dict,
            );
            if !opts_dict.is_null() {
                av_dict_free(&mut opts_dict);
            }
            if ret < 0 {
                avcodec_free_context(&mut self.codec_ctx);
                anyhow::bail!("Failed to open decoder: {}", ffmpeg_strerror(ret));
            }
        }

        Ok(())
    }

    pub fn get_next_subtitle_frame(&self) -> Option<SubtitleFrame> {
        if self.codec_ctx.is_null() || self.format_ctx.is_null() {
            return None;
        }

        let mut packet = unsafe { av_packet_alloc() };
        if packet.is_null() {
            return None;
        }

        let result = self.get_next_subtitle_frame_inner(packet);
        unsafe {
            av_packet_free(&mut packet);
        }
        result
    }

    fn get_next_subtitle_frame_inner(&self, packet: *mut AVPacket) -> Option<SubtitleFrame> {
        unsafe {
            while av_read_frame(self.format_ctx, packet) >= 0 {
                if (*packet).stream_index != self.subtitle_stream_index {
                    av_packet_unref(packet);
                    continue;
                }

                let mut subtitle = std::mem::zeroed::<AVSubtitle>();
                let mut got_subtitle: c_int = 0;
                let ret = avcodec_decode_subtitle2(
                    self.codec_ctx,
                    &mut subtitle,
                    &mut got_subtitle,
                    packet,
                );

                if ret < 0 {
                    eprintln!("Warning: subtitle decode error: {}", ffmpeg_strerror(ret));
                    av_packet_unref(packet);
                    continue;
                }

                if got_subtitle == 0 {
                    avsubtitle_free(&mut subtitle);
                    av_packet_unref(packet);
                    continue;
                }

                let stream = *(*self.format_ctx)
                    .streams
                    .add(self.subtitle_stream_index as usize);
                let time_base = (*stream).time_base;
                let pts = if (*packet).pts != AV_NOPTS_VALUE {
                    (*packet).pts
                } else {
                    subtitle.pts
                };
                let base_timestamp = pts_to_seconds(pts, time_base);
                let start_time = if subtitle.start_display_time != INVALID_DISPLAY_TIME
                    && subtitle.end_display_time != INVALID_DISPLAY_TIME
                {
                    base_timestamp + (subtitle.start_display_time as f64 / 1000.0)
                } else {
                    base_timestamp
                };
                let end_time = if subtitle.start_display_time != INVALID_DISPLAY_TIME
                    && subtitle.end_display_time != INVALID_DISPLAY_TIME
                {
                    base_timestamp + (subtitle.end_display_time as f64 / 1000.0)
                } else {
                    base_timestamp
                };

                if subtitle.num_rects == 0 {
                    avsubtitle_free(&mut subtitle);
                    av_packet_unref(packet);
                    return Some(SubtitleFrame {
                        bitmap: None,
                        pts,
                        timestamp: base_timestamp,
                        start_time,
                        end_time,
                        x: 0,
                        y: 0,
                    });
                }

                let mut min_x = i32::MAX;
                let mut min_y = i32::MAX;
                let mut max_x = i32::MIN;
                let mut max_y = i32::MIN;
                let mut has_bitmap = false;

                for i in 0..(subtitle.num_rects as usize) {
                    let rect_ptr = *subtitle.rects.add(i);
                    if rect_ptr.is_null() {
                        continue;
                    }
                    let rect = &*rect_ptr;
                    if rect.type_ == AVSubtitleType_SUBTITLE_BITMAP {
                        has_bitmap = true;
                        min_x = min_x.min(rect.x);
                        min_y = min_y.min(rect.y);
                        max_x = max_x.max(rect.x + rect.w);
                        max_y = max_y.max(rect.y + rect.h);
                    }
                }

                if !has_bitmap {
                    avsubtitle_free(&mut subtitle);
                    av_packet_unref(packet);
                    continue;
                }

                let composite_width = max_x - min_x;
                let composite_height = max_y - min_y;
                let stride = composite_width * 4;
                let mut data = vec![0u8; (stride * composite_height) as usize];

                for i in 0..(subtitle.num_rects as usize) {
                    let rect_ptr = *subtitle.rects.add(i);
                    if rect_ptr.is_null() {
                        continue;
                    }
                    let rect = &*rect_ptr;
                    if rect.type_ != AVSubtitleType_SUBTITLE_BITMAP {
                        continue;
                    }
                    if rect.data[0].is_null() || rect.data[1].is_null() {
                        continue;
                    }

                    let indices = std::slice::from_raw_parts(
                        rect.data[0],
                        (rect.linesize[0] * rect.h) as usize,
                    );
                    let palette = std::slice::from_raw_parts(
                        rect.data[1] as *const u32,
                        rect.nb_colors as usize,
                    );
                    let dest_x = rect.x - min_x;
                    let dest_y = rect.y - min_y;
                    let line0 = rect.linesize[0] as usize;

                    for y in 0..(rect.h as usize) {
                        for x in 0..(rect.w as usize) {
                            let idx = indices[y * line0 + x] as usize;
                            if idx >= palette.len() {
                                continue;
                            }
                            let argb = palette[idx];
                            let r = ((argb >> 16) & 0xFF) as u8;
                            let g = ((argb >> 8) & 0xFF) as u8;
                            let b = (argb & 0xFF) as u8;
                            let a = ((argb >> 24) & 0xFF) as u8;

                            let comp_x = dest_x + x as i32;
                            let comp_y = dest_y + y as i32;
                            if comp_x >= 0
                                && comp_x < composite_width
                                && comp_y >= 0
                                && comp_y < composite_height
                            {
                                let offset =
                                    ((comp_y * composite_width + comp_x) * 4) as usize;
                                if a > 0 {
                                    if a == 255 || data[offset + 3] == 0 {
                                        data[offset] = r;
                                        data[offset + 1] = g;
                                        data[offset + 2] = b;
                                        data[offset + 3] = a;
                                    } else {
                                        let alpha = a as f32 / 255.0;
                                        let inv = 1.0 - alpha;
                                        data[offset] =
                                            (r as f32 * alpha + data[offset] as f32 * inv) as u8;
                                        data[offset + 1] =
                                            (g as f32 * alpha + data[offset + 1] as f32 * inv) as u8;
                                        data[offset + 2] =
                                            (b as f32 * alpha + data[offset + 2] as f32 * inv) as u8;
                                        data[offset + 3] =
                                            (a as f32 + data[offset + 3] as f32 * inv) as u8;
                                    }
                                }
                            }
                        }
                    }
                }

                avsubtitle_free(&mut subtitle);
                av_packet_unref(packet);

                return Some(SubtitleFrame {
                    bitmap: Some(BitmapData {
                        data,
                        width: composite_width,
                        height: composite_height,
                        stride,
                    }),
                    pts,
                    timestamp: base_timestamp,
                    start_time,
                    end_time,
                    x: min_x,
                    y: min_y,
                });
            }
        }
        None
    }

    pub fn close(&mut self) {
        unsafe {
            if !self.codec_ctx.is_null() {
                avcodec_free_context(&mut self.codec_ctx);
                self.codec_ctx = ptr::null_mut();
            }
            if !self.format_ctx.is_null() {
                avformat_close_input(&mut self.format_ctx);
                self.format_ctx = ptr::null_mut();
            }
        }
        self.subtitle_stream_index = -1;
    }
}

impl Drop for FfmpegWrapper {
    fn drop(&mut self) {
        self.close();
    }
}
