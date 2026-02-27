//! Output resolution (canvas_size), libaribcaption defaults.

use std::collections::HashMap;

/// Default output resolution.
const DEFAULT_CANVAS: &str = "1920x1080";

#[inline]
fn debug_eprint(debug: bool, msg: &str) {
    if debug {
        eprintln!("{}", msg);
    }
}

/// Determine canvas_size from video dimensions and anamorphic flag.
/// 720x480 → 720x480. 1280x720 → 1280x720. 1440x1080 with --anamorphic → 1440x1080. Otherwise 1920x1080.
pub fn determine_canvas_size(
    video_width: i32,
    video_height: i32,
    anamorphic: bool,
    debug: bool,
) -> anyhow::Result<String> {
    let canvas = match (video_width, video_height) {
        (0, 0) => DEFAULT_CANVAS,
        (1920, 1080) => DEFAULT_CANVAS,
        (1280, 720) => {
            debug_eprint(debug, "canvas_size: 1280x720");
            "1280x720"
        }
        (1440, 1080) => {
            if anamorphic {
                debug_eprint(debug, "canvas_size: 1440x1080 (anamorphic, source 1440x1080)");
                "1440x1080"
            } else {
                DEFAULT_CANVAS
            }
        }
        (720, 480) => {
            debug_eprint(debug, "canvas_size: 720x480");
            "720x480"
        }
        _ => anyhow::bail!(
            "Unsupported video resolution: {}x{}. Supported: 1920x1080, 1440x1080, 1280x720, 720x480.",
            video_width,
            video_height
        ),
    };
    if debug && canvas == DEFAULT_CANVAS && (video_width != 0 || video_height != 0) {
        eprintln!("canvas_size: {}", canvas);
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

/// Default font for libaribcaption: Windows uses Rounded M+ only; others use Hiragino + Rounded M+.
#[cfg(target_os = "windows")]
fn default_arib_font() -> String {
    "Rounded M+ 1m for ARIB".to_string()
}
#[cfg(not(target_os = "windows"))]
fn default_arib_font() -> String {
    "Hiragino Maru Gothic ProN, Rounded M+ 1m for ARIB".to_string()
}

/// Insert libaribcaption default options only for keys that are not already set.
pub fn setup_libaribcaption_defaults(opts: &mut HashMap<String, String>) {
    opts.entry("caption_encoding".to_string())
        .or_insert_with(|| "0".to_string());
    opts.entry("font".to_string())
        .or_insert_with(default_arib_font);
    opts.entry("force_outline_text".to_string())
        .or_insert_with(|| "0".to_string());
    opts.entry("ignore_background".to_string())
        .or_insert_with(|| "0".to_string());
    opts.entry("ignore_ruby".to_string())
        .or_insert_with(|| "0".to_string());
    opts.entry("outline_width".to_string())
        .or_insert_with(|| "0.0".to_string());
    opts.entry("replace_drcs".to_string())
        .or_insert_with(|| "0".to_string());
    opts.entry("replace_msz_ascii".to_string())
        .or_insert_with(|| "0".to_string());
    opts.entry("replace_msz_japanese".to_string())
        .or_insert_with(|| "0".to_string());
    opts.entry("replace_msz_glyph".to_string())
        .or_insert_with(|| "0".to_string());
}

