mod bdn;
mod bitmap;
mod config;
mod ffmpeg;
mod ffmpeg_sys;
mod options;

use std::collections::HashMap;
use std::path::Path;

use clap::Parser;

use bdn::{adjust_timestamp, determine_video_format, time_to_tc, BdnInfo, BdnXmlGenerator, SubtitleEvent};
use bitmap::{generate_png_filename, save_bitmap_as_png};
use config::{
    adjust_timestamp_for_range, determine_canvas_size, parse_canvas_size,
    setup_libaribcaption_defaults,
};
use ffmpeg::{FfmpegWrapper, SubtitleFrame};
use options::{parse_libaribcaption_opts, parse_time_string};

const VERSION: &str = "0.1.1";

#[derive(Parser)]
#[command(name = "arib2bdnxml")]
#[command(version = VERSION)]
#[command(about = "Extract ARIB subtitles from .ts/.m2ts and generate BDN XML + PNG using libaribcaption (via FFmpeg)")]
struct Cli {
    #[arg(short, long, value_name = "RESOLUTION")]
    resolution: Option<String>,

    #[arg(long, value_name = "OPTIONS")]
    libaribcaption_opt: Vec<String>,

    #[arg(long, value_name = "DIR")]
    output: Option<String>,

    #[arg(long, value_name = "TIME")]
    ss: Option<String>,

    #[arg(long, value_name = "TIME")]
    to: Option<String>,

    #[arg(long)]
    debug: bool,

    #[arg(help = "Input file (.ts / .m2ts)")]
    input_file: Option<String>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let input_file = match &cli.input_file {
        Some(f) if !f.is_empty() && f != "-h" && f != "--help" && f != "-v" && f != "--version" => {
            f.clone()
        }
        _ => {
            print_help();
            if cli.input_file.as_deref() == Some("-h") || cli.input_file.as_deref() == Some("--help") {
                std::process::exit(0);
            }
            if cli.input_file.as_deref() == Some("-v") || cli.input_file.as_deref() == Some("--version") {
                print_version();
                std::process::exit(0);
            }
            anyhow::bail!("Input file not specified.");
        }
    };

    if !Path::new(&input_file).exists() {
        anyhow::bail!("Input file does not exist: {}", input_file);
    }

    let mut libaribcaption_opts = HashMap::new();
    for s in &cli.libaribcaption_opt {
        for (k, v) in parse_libaribcaption_opts(s) {
            libaribcaption_opts.insert(k, v);
        }
    }

    let ss = cli
        .ss
        .as_ref()
        .map(|s| parse_time_string(s).map_err(|e| anyhow::anyhow!("{}", e)))
        .transpose()?;
    let to = cli
        .to
        .as_ref()
        .map(|s| parse_time_string(s).map_err(|e| anyhow::anyhow!("{}", e)))
        .transpose()?;

    if cli.debug {
        if let Some(s) = &cli.ss {
            if let Ok(v) = parse_time_string(s) {
                eprintln!("DEBUG: --ss parsed: '{}' -> {}s", s, v);
            }
        }
        if let Some(s) = &cli.to {
            if let Ok(v) = parse_time_string(s) {
                eprintln!("DEBUG: --to parsed: '{}' -> {}s", s, v);
            }
        }
    }

    let base_name = Path::new(&input_file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output")
        .to_string();

    let output_dir = match &cli.output {
        Some(d) => d.clone(),
        None => {
            let parent = Path::new(&input_file).parent().unwrap_or(Path::new("."));
            parent.join(format!("{}_bdnxml", base_name)).display().to_string()
        }
    };

    std::fs::create_dir_all(&output_dir)?;

    let mut ffmpeg = FfmpegWrapper::new();
    ffmpeg.set_debug(cli.debug);
    ffmpeg.open_file(&input_file, ss, to)?;

    let video_info = ffmpeg.get_video_info();
    let canvas_size = determine_canvas_size(
        &cli.resolution,
        video_info.width,
        video_info.height,
        cli.debug,
    )?;
    libaribcaption_opts.insert("canvas_size".to_string(), canvas_size.clone());
    setup_libaribcaption_defaults(&mut libaribcaption_opts);

    let (canvas_width, canvas_height) = parse_canvas_size(&canvas_size)?;
    let fps = if video_info.fps > 0.0 {
        video_info.fps
    } else {
        29.97
    };
    let video_format = determine_video_format(canvas_height, video_info.is_interlaced);

    let bdn_info = BdnInfo {
        video_width: canvas_width,
        video_height: canvas_height,
        fps,
        video_format: video_format.to_string(),
    };

    ffmpeg.init_decoder(&libaribcaption_opts)?;

    let mut generator = BdnXmlGenerator::new(bdn_info.clone());
    let mut events: Vec<SubtitleEvent> = Vec::new();
    let mut frame_index: usize = 0;

    let mut subtitle_frame = match ffmpeg.get_next_subtitle_frame() {
        Some(f) => f,
        None => {
            if cli.debug {
                eprintln!("No subtitle frames found.");
            }
            let xml_path = Path::new(&output_dir).join(format!("{}.xml", base_name));
            generator.write_to_file(xml_path.to_str().unwrap())?;
            return Ok(());
        }
    };

    let mut next_frame = ffmpeg.get_next_subtitle_frame();

    loop {
        if cli.debug {
            eprintln!("Subtitle frame: index {}", frame_index);
        }

        if subtitle_frame.bitmap.is_none() && subtitle_frame.timestamp > 0.0 {
            if let Some(last) = events.last_mut() {
                let mut clear_ts = adjust_timestamp(subtitle_frame.timestamp, video_info.start_time);
                if let Some(t) = to {
                    if clear_ts > t {
                        clear_ts = t;
                    }
                }
                if let Some(s) = ss {
                    clear_ts -= s;
                }
                last.out_tc = time_to_tc(clear_ts, bdn_info.fps);
            }
            if !advance_to_next_frame(&mut subtitle_frame, &mut next_frame, &ffmpeg) {
                break;
            }
            continue;
        }

        if subtitle_frame.bitmap.is_none() {
            if !advance_to_next_frame(&mut subtitle_frame, &mut next_frame, &ffmpeg) {
                break;
            }
            continue;
        }

        let bitmap = subtitle_frame.bitmap.as_ref().unwrap();
        if bitmap.width == 0 || bitmap.height == 0 {
            if !advance_to_next_frame(&mut subtitle_frame, &mut next_frame, &ffmpeg) {
                break;
            }
            continue;
        }

        let adjusted_start = if subtitle_frame.start_time > 0.0
            && subtitle_frame.end_time > subtitle_frame.start_time
        {
            adjust_timestamp(subtitle_frame.start_time, video_info.start_time)
        } else {
            adjust_timestamp(subtitle_frame.timestamp, video_info.start_time)
        };

        let adjusted_end = if subtitle_frame.start_time > 0.0
            && subtitle_frame.end_time > subtitle_frame.start_time
        {
            adjust_timestamp(subtitle_frame.end_time, video_info.start_time)
        } else if let Some(ref next) = next_frame {
            if next.bitmap.is_some() {
                if next.start_time > 0.0 && next.end_time > next.start_time {
                    adjust_timestamp(next.start_time, video_info.start_time)
                } else {
                    adjust_timestamp(next.timestamp, video_info.start_time)
                }
            } else {
                adjust_timestamp(next.timestamp, video_info.start_time)
            }
        } else {
            adjusted_start + 1.0
        };

        let (mut adj_start, mut adj_end) = (adjusted_start, adjusted_end);
        if !adjust_timestamp_for_range(&mut adj_start, &mut adj_end, ss, to, cli.debug) {
            if !advance_to_next_frame(&mut subtitle_frame, &mut next_frame, &ffmpeg) {
                break;
            }
            continue;
        }

        if adj_start >= adj_end {
            if !advance_to_next_frame(&mut subtitle_frame, &mut next_frame, &ffmpeg) {
                break;
            }
            continue;
        }

        let png_filename = generate_png_filename(frame_index, &base_name);
        let png_path = Path::new(&output_dir).join(&png_filename);
        if save_bitmap_as_png(bitmap, png_path.to_str().unwrap()).is_err() {
            eprintln!("Warning: failed to save PNG: {}", png_path.display());
            if !advance_to_next_frame(&mut subtitle_frame, &mut next_frame, &ffmpeg) {
                break;
            }
            continue;
        }

        events.push(SubtitleEvent {
            in_tc: time_to_tc(adj_start, bdn_info.fps),
            out_tc: time_to_tc(adj_end, bdn_info.fps),
            png_file: png_filename,
            x: subtitle_frame.x,
            y: subtitle_frame.y,
            width: bitmap.width,
            height: bitmap.height,
        });
        frame_index += 1;

        if !advance_to_next_frame(&mut subtitle_frame, &mut next_frame, &ffmpeg) {
            break;
        }
    }

    for event in &events {
        generator.add_event(event);
    }

    let xml_path = Path::new(&output_dir).join(format!("{}.xml", base_name));
    generator.write_to_file(xml_path.to_str().unwrap())?;

    if cli.debug {
        eprintln!("Done: processed {} subtitle events.", events.len());
        eprintln!("Output: {}", xml_path.display());
    }

    Ok(())
}

/// Advance to the next subtitle frame. Returns true if advanced, false if no more frames.
fn advance_to_next_frame(
    subtitle_frame: &mut SubtitleFrame,
    next_frame: &mut Option<SubtitleFrame>,
    ffmpeg: &FfmpegWrapper,
) -> bool {
    if let Some(sf) = next_frame.take() {
        *subtitle_frame = sf;
        *next_frame = ffmpeg.get_next_subtitle_frame();
        true
    } else {
        false
    }
}

fn print_help() {
    eprintln!(
        r#"Usage: arib2bdnxml [OPTIONS] <INPUT_FILE>

Options:
  -r, --resolution <RES>       Output resolution (1920x1080, 1440x1080, 1280x720, 720x480)
  --libaribcaption-opt <OPTS>  libaribcaption options (key=value,key=value)
  --output <DIR>               Output directory
  --ss <TIME>                  Start time for timestamp adjustment
  --to <TIME>                  End time for timestamp adjustment
  --debug                      Enable debug logging
  -h, --help                   Show this help
  -v, --version                Show version
"#
    );
}

fn print_version() {
    println!("arib2bdnxml {}", VERSION);
}
