mod bdn;
mod bitmap;
mod config;
mod ffmpeg;
mod ffmpeg_sys;
mod options;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use clap::Parser;

use bdn::{adjust_timestamp, time_to_tc, BdnInfo, BdnXmlGenerator, SubtitleEvent};
use bitmap::{generate_png_filename, save_bitmap_as_png};
use config::{determine_canvas_size, setup_libaribcaption_defaults};
use ffmpeg::{probe_video_resolution, FfmpegWrapper, SubtitleFrame};
use options::parse_libaribcaption_opts;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Derives candidate base names for companion .mkv from .mks stem.
/// Strips from the right: .forced, .jpn/.eng, then .NN (track number).
/// e.g. "MOVIE.01.jpn.forced" -> ["MOVIE.01.jpn.forced", "MOVIE.01.jpn", "MOVIE.01", "MOVIE"]
/// so that we try MOVIE.mkv, MOVIE.01.mkv, ... and match MOVIE.mkv.
fn companion_mkv_base_candidates(stem: &str) -> Vec<String> {
    if stem.is_empty() {
        return vec![];
    }
    let mut out = vec![stem.to_string()];
    let mut rest = stem;
    while let Some(trimmed) = rest
        .strip_suffix(".forced")
        .or_else(|| rest.strip_suffix(".jpn"))
        .or_else(|| rest.strip_suffix(".eng"))
        .or_else(|| rest.strip_suffix(".japanese"))
        .or_else(|| rest.strip_suffix(".english"))
    {
        rest = trimmed;
        if !rest.is_empty() {
            out.push(rest.to_string());
        }
    }
    while let Some(trimmed) = strip_trailing_digits(rest) {
        rest = trimmed;
        if !rest.is_empty() {
            out.push(rest.to_string());
        }
    }
    out.dedup();
    out
}

/// Strips trailing .NN (e.g. .01, .001) from the end of s.
fn strip_trailing_digits(s: &str) -> Option<&str> {
    let t = s.trim_end_matches(|c: char| c.is_ascii_digit());
    (t.len() < s.len() && t.ends_with('.')).then(|| t.strip_suffix('.').unwrap_or(t))
}

/// Resolve effective video resolution: from video_info if present, else from companion .mkv when anamorphic.
fn resolve_effective_resolution(
    input_file: &str,
    video_width: i32,
    video_height: i32,
    anamorphic: bool,
    debug: bool,
) -> (i32, i32) {
    if video_width != 0 || video_height != 0 {
        return (video_width, video_height);
    }
    if !anamorphic {
        return (0, 0);
    }
    let input_path = Path::new(input_file);
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let parent = input_path.parent().unwrap_or(Path::new("."));
    let base_names = companion_mkv_base_candidates(stem);
    let mut mkv_candidates: Vec<PathBuf> = Vec::new();
    for base in &base_names {
        mkv_candidates.push(parent.join(format!("{}.mkv", base)));
        if let Some(gp) = parent.parent() {
            mkv_candidates.push(gp.join(format!("{}.mkv", base)));
        }
    }
    for path in &mkv_candidates {
        if path.exists() {
            if let Ok((w, h)) = probe_video_resolution(path.to_str().unwrap_or("")) {
                if (w, h) == (1440, 1080) || (w, h) == (1280, 720) || (w, h) == (720, 480) {
                    if debug {
                        eprintln!("Companion .mkv resolution: {}x{} ({})", w, h, path.display());
                    }
                    return (w, h);
                }
            }
        }
    }
    (0, 0)
}

/// Map canvas_size string to BDN video_format.
fn video_format_from_canvas(canvas_size: &str) -> String {
    match canvas_size {
        "720x480" => "ntsc".to_string(),
        "1280x720" => "720p".to_string(),
        "1440x1080" => "1440x1080".to_string(),
        _ => "1080p".to_string(),
    }
}

#[derive(Parser)]
#[command(name = "arib2bdnxml")]
#[command(version = VERSION)]
#[command(about = "Extract ARIB subtitles from .ts/.m2ts/.mkv/.mks and generate BDN XML + PNG using libaribcaption (via FFmpeg)")]
struct Cli {
    #[arg(short, long)]
    anamorphic: bool,

    #[arg(long = "arib-params", value_name = "OPTIONS")]
    arib_params: Vec<String>,

    #[arg(short, long, value_name = "DIR")]
    output: Option<String>,

    #[arg(short, long)]
    debug: bool,

    #[arg(help = "Input file (.ts, .m2ts, .mkv, .mks)")]
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
    for s in &cli.arib_params {
        for (k, v) in parse_libaribcaption_opts(s) {
            libaribcaption_opts.insert(k, v);
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
    ffmpeg.open_file(&input_file)?;

    let video_info = ffmpeg.get_video_info();
    let (effective_width, effective_height) = resolve_effective_resolution(
        &input_file,
        video_info.width,
        video_info.height,
        cli.anamorphic,
        cli.debug,
    );
    let canvas_size = determine_canvas_size(
        effective_width,
        effective_height,
        cli.anamorphic,
        cli.debug,
    )?;
    libaribcaption_opts.insert("canvas_size".to_string(), canvas_size.clone());
    setup_libaribcaption_defaults(&mut libaribcaption_opts);

    let fps = if video_info.fps > 0.0 {
        video_info.fps
    } else {
        29.97
    };
    let video_format = video_format_from_canvas(&canvas_size);
    let bdn_info = BdnInfo {
        fps,
        video_format,
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
                let clear_ts = adjust_timestamp(subtitle_frame.timestamp, video_info.start_time);
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

        if adjusted_start >= adjusted_end {
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
            in_tc: time_to_tc(adjusted_start, bdn_info.fps),
            out_tc: time_to_tc(adjusted_end, bdn_info.fps),
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
  -a, --anamorphic             Use anamorphic output for 1440x1080 (→ 1440x1080)
  --arib-params <OPTS>          libaribcaption options (key=value,key=value)
  --output, -o <DIR>            Output directory
  --debug, -d                   Enable debug logging
  -h, --help                   Show this help
  -v, --version                Show version
"#
    );
}

fn print_version() {
    println!("arib2bdnxml {}", VERSION);
}

#[cfg(test)]
mod tests {
    use super::companion_mkv_base_candidates;

    #[test]
    fn test_companion_mkv_base_candidates() {
        assert!(companion_mkv_base_candidates("").is_empty());
        let c = companion_mkv_base_candidates("MOVIE.jpn");
        assert!(c.contains(&"MOVIE".to_string()));
        assert!(c.contains(&"MOVIE.jpn".to_string()));
        let c = companion_mkv_base_candidates("MOVIE.01.jpn");
        assert!(c.contains(&"MOVIE".to_string()));
        assert!(c.contains(&"MOVIE.01".to_string()));
        assert!(c.contains(&"MOVIE.01.jpn".to_string()));
        let c = companion_mkv_base_candidates("MOVIE.01.jpn.forced");
        assert!(c.contains(&"MOVIE".to_string()));
        assert!(c.contains(&"MOVIE.01.jpn".to_string()));
        let c = companion_mkv_base_candidates("MOVIE.forced");
        assert!(c.contains(&"MOVIE".to_string()));
    }
}
