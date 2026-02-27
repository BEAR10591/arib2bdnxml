// Detect FFmpeg via FFMPEG_DIR, pkg-config, or PATH.
// Requires FFmpeg 8.0+ built with --enable-libaribcaption.
// Version and libaribcaption are checked by running ffmpeg -version and ffmpeg -decoders.
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    println!("cargo:rerun-if-env-changed=PATH");
    let (include_paths, link_search, root): (Vec<PathBuf>, Option<PathBuf>, Option<PathBuf>) =
        if let Ok(dir) = env::var("FFMPEG_DIR") {
            let root = PathBuf::from(&dir);
            let inc = root.join("include");
            let lib = root.join("lib");
            (vec![inc], Some(lib), Some(root))
        } else if let Ok((incs, lib_path)) = try_pkg_config() {
            (incs, lib_path, None)
        } else if let Some(root) = find_ffmpeg_from_path() {
            let inc = root.join("include");
            let lib = root.join("lib");
            if inc.exists() && lib.exists() {
                (vec![inc], Some(lib), Some(root))
            } else {
                panic!(
                    "FFmpeg found on PATH at {} but missing include/ or lib/. Set FFMPEG_DIR (see README).",
                    root.display()
                );
            }
        } else {
            panic!(
                "FFmpeg not found. Set FFMPEG_DIR, use pkg-config, or ensure ffmpeg is on PATH (see README)."
            );
        };

    if let Some(lib) = &link_search {
        println!("cargo:rustc-link-search=native={}", lib.display());
    }

    let ffmpeg_bin = get_ffmpeg_binary(&root);
    check_ffmpeg_version(&ffmpeg_bin);
    check_libaribcaption(&ffmpeg_bin);

    let mut clang_args = Vec::new();
    for inc in &include_paths {
        clang_args.push(format!("-I{}", inc.display()));
    }

    const WRAPPER_H: &str = r#"
#include <libavutil/error.h>
#include <libavutil/log.h>
#include <libavutil/rational.h>
#include <libavutil/pixfmt.h>
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>
"#;

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let bindings = bindgen::Builder::default()
        .header_contents("wrapper.h", WRAPPER_H)
        .clang_args(&clang_args)
        .derive_default(true)
        .derive_debug(true)
        .layout_tests(false)
        .generate()
        .expect("Failed to generate FFmpeg bindings");

    bindings
        .write_to_file(out_dir.join("ffmpeg.rs"))
        .expect("Failed to write ffmpeg.rs");

    println!("cargo:rustc-link-lib=avformat");
    println!("cargo:rustc-link-lib=avcodec");
    println!("cargo:rustc-link-lib=avutil");
}

fn get_ffmpeg_binary(root: &Option<PathBuf>) -> PathBuf {
    let exe = if env::consts::OS == "windows" {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    if let Some(r) = root {
        let bin = r.join("bin").join(exe);
        if bin.exists() {
            return bin;
        }
    }
    let cmd = if env::consts::OS == "windows" { "where" } else { "which" };
    let out = Command::new(cmd)
        .arg(exe)
        .output()
        .expect("Failed to run which/where for ffmpeg");
    let first_line = std::str::from_utf8(&out.stdout)
        .expect("ffmpeg path not utf-8")
        .lines()
        .next()
        .expect("ffmpeg not found on PATH")
        .trim();
    PathBuf::from(first_line)
}

fn decode_utf16_le(buf: &[u8]) -> Option<String> {
    if buf.len() < 2 {
        return Some(String::new());
    }
    let mut u16s = Vec::with_capacity(buf.len() / 2);
    let mut i = 0;
    if buf.len() >= 2 && buf[0] == 0xFF && buf[1] == 0xFE {
        i = 2;
    }
    while i + 1 < buf.len() {
        u16s.push(u16::from_le_bytes([buf[i], buf[i + 1]]));
        i += 2;
    }
    String::from_utf16(&u16s).ok()
}

fn parse_major_from_version_output(combined: &str) -> u32 {
    for line in combined.lines() {
        let line = line.trim();
        if let Some(v) = line
            .strip_prefix("ffmpeg version ")
            .or_else(|| line.strip_prefix("FFmpeg version "))
        {
            if let Some(m) = v.split('.').next().and_then(|s| s.parse::<u32>().ok()) {
                return m;
            }
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 && parts[1].eq_ignore_ascii_case("version") {
            if let Some(m) = parts[2].split('.').next().and_then(|s| s.parse::<u32>().ok()) {
                return m;
            }
        }
    }
    for line in combined.lines() {
        if let Some(pos) = line.find("8.") {
            let tail = &line[pos..];
            if let Some(first) = tail.split(|c: char| !c.is_ascii_digit() && c != '.').next() {
                if let Some(m) = first.split('.').next().and_then(|s| s.parse::<u32>().ok()) {
                    if m >= 8 {
                        return m;
                    }
                }
            }
        }
    }
    0
}

fn check_ffmpeg_version(ffmpeg_bin: &Path) {
    let out = Command::new(ffmpeg_bin)
        .arg("-version")
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {}: {}", ffmpeg_bin.display(), e));
    let out_str = String::from_utf8_lossy(&out.stdout);
    let err_str = String::from_utf8_lossy(&out.stderr);
    let mut combined = format!("{}\n{}", out_str, err_str);
    if combined.trim().is_empty() || !combined.contains('8') {
        if let (Some(sout), Some(serr)) =
            (decode_utf16_le(&out.stdout), decode_utf16_le(&out.stderr))
        {
            combined = format!("{}\n{}", sout, serr);
        }
    }
    let major = parse_major_from_version_output(&combined);
    if major < 8 {
        let detected = combined
            .lines()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.trim())
            .unwrap_or("unknown");
        panic!(
            "FFmpeg 8.0 or newer is required (detected: {}). Install FFmpeg 8.0+ with --enable-libaribcaption (see README).",
            detected
        );
    }
}

fn check_libaribcaption(ffmpeg_bin: &Path) {
    let out = Command::new(ffmpeg_bin)
        .args(["-hide_banner", "-decoders"])
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {} -decoders: {}", ffmpeg_bin.display(), e));
    let out_utf8 = String::from_utf8_lossy(&out.stdout);
    let err_utf8 = String::from_utf8_lossy(&out.stderr);
    let out_str = if !out_utf8.trim().is_empty() {
        out_utf8.to_string()
    } else if !err_utf8.trim().is_empty() {
        err_utf8.to_string()
    } else if let Some(s) = decode_utf16_le(&out.stderr) {
        s
    } else if let Some(s) = decode_utf16_le(&out.stdout) {
        s
    } else {
        String::new()
    };
    let has_libaribcaption = out_str
        .lines()
        .any(|line| line.contains("libaribcaption") && line.contains("arib_caption"));
    if !has_libaribcaption {
        panic!(
            "FFmpeg was not built with --enable-libaribcaption. \
             Run `ffmpeg -hide_banner -decoders | grep libaribcaption` to verify. \
             Use an FFmpeg 8.0+ build with libaribcaption enabled (see README)."
        );
    }
}

fn try_pkg_config() -> Result<(Vec<PathBuf>, Option<PathBuf>), ()> {
    let mut incs = Vec::new();
    let mut lib_path = None::<PathBuf>;
    for lib in &["libavcodec", "libavformat", "libavutil"] {
        let lib = pkg_config::Config::new()
            .atleast_version(match *lib {
                "libavcodec" | "libavformat" => "58.0.0",
                "libavutil" => "56.0.0",
                _ => "0.0.0",
            })
            .probe(lib)
            .map_err(|_| ())?;
        incs.extend(lib.include_paths);
        if lib_path.is_none() {
            lib_path = lib.link_paths.into_iter().next();
        }
    }
    incs.sort();
    incs.dedup();
    Ok((incs, lib_path))
}

fn find_ffmpeg_from_path() -> Option<PathBuf> {
    let exe = if env::consts::OS == "windows" {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let out = Command::new(if env::consts::OS == "windows" {
        "where"
    } else {
        "which"
    })
    .arg(exe)
    .output()
    .ok()?;
    if !out.status.success() {
        return None;
    }
    let first_line = std::str::from_utf8(&out.stdout)
        .ok()?
        .lines()
        .next()?
        .trim();
    let path = Path::new(first_line)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(first_line));
    let bin = path.parent()?;
    let root = bin.parent()?;
    Some(root.to_path_buf())
}
