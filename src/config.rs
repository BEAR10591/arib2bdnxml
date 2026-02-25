//! Resolution, canvas_size, libaribcaption defaults, and time range configuration.

use std::collections::HashMap;

/// Valid resolution strings.
pub const VALID_RESOLUTIONS: &[&str] = &["1920x1080", "1440x1080", "1280x720", "720x480"];

/// Determine canvas_size string from resolution option or video dimensions.
pub fn determine_canvas_size(
    resolution: &Option<String>,
    video_width: i32,
    video_height: i32,
    debug: bool,
) -> anyhow::Result<String> {
    if let Some(ref res) = resolution {
        if VALID_RESOLUTIONS.contains(&res.as_str()) {
            if debug {
                eprintln!("canvas_size from --resolution: {}", res);
            }
            return Ok(res.clone());
        }
        anyhow::bail!(
            "Invalid resolution: {}. Valid: 1920x1080, 1440x1080, 1280x720, 720x480",
            res
        );
    }
    let canvas = match (video_width, video_height) {
        (1920, 1080) | (1440, 1080) => "1920x1080",
        (1280, 720) => "1280x720",
        (720, 480) => "720x480",
        _ => anyhow::bail!(
            "Unsupported video resolution: {}x{}. Use --resolution to specify.",
            video_width,
            video_height
        ),
    };
    if debug {
        eprintln!("canvas_size auto-detected from video: {}", canvas);
    }
    Ok(canvas.to_string())
}

/// Parse a "WxH" string into (width, height).
pub fn parse_canvas_size(s: &str) -> anyhow::Result<(i32, i32)> {
    let mut it = s.split('x');
    let w: i32 = it
        .next()
        .ok_or_else(|| anyhow::anyhow!("invalid canvas_size format"))?
        .trim()
        .parse()?;
    let h: i32 = it
        .next()
        .ok_or_else(|| anyhow::anyhow!("invalid canvas_size format"))?
        .trim()
        .parse()?;
    if it.next().is_some() {
        anyhow::bail!("invalid canvas_size format: {}", s);
    }
    Ok((w, h))
}

/// Insert libaribcaption default options only for keys that are not already set.
pub fn setup_libaribcaption_defaults(opts: &mut HashMap<String, String>) {
    opts.entry("outline_width".to_string())
        .or_insert_with(|| "0.0".to_string());
    opts.entry("replace_msz_ascii".to_string())
        .or_insert_with(|| "0".to_string());
    opts.entry("replace_msz_japanese".to_string())
        .or_insert_with(|| "0".to_string());
    opts.entry("replace_drcs".to_string())
        .or_insert_with(|| "0".to_string());
}

/// Adjust start/end timestamps for --ss/--to range; returns true if within range.
pub fn adjust_timestamp_for_range(
    adjusted_start: &mut f64,
    adjusted_end: &mut f64,
    ss: Option<f64>,
    to: Option<f64>,
    debug: bool,
) -> bool {
    if let Some(s) = ss {
        if *adjusted_start < s {
            if debug {
                eprintln!("Skipping subtitle before --ss");
            }
            return false;
        }
    }
    if let Some(t) = to {
        if *adjusted_start >= t {
            if debug {
                eprintln!("Skipping subtitle past --to");
            }
            return false;
        }
        if *adjusted_end > t {
            *adjusted_end = t;
        }
    }
    if let Some(s) = ss {
        *adjusted_start -= s;
        *adjusted_end -= s;
    }
    true
}
