// Detect FFmpeg via FFMPEG_DIR, pkg-config, or PATH (ffmpeg executable).
// - macOS: expect Homebrew tap bear10591/tap/ffmpeg (libaribcaption enabled).
// - Windows: expect Gyan.dev FFmpeg full build (with dev headers/libs); winget or PATH.
// Requires FFmpeg 8.0+ built with --enable-libaribcaption.
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

    // Minimal FFmpeg includes for ARIB subtitle decoding (no avfft.h).
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

    // Link with FFmpeg libs (order can matter on some platforms)
    println!("cargo:rustc-link-lib=avformat");
    println!("cargo:rustc-link-lib=avcodec");
    println!("cargo:rustc-link-lib=avutil");
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
    // .../bin/ffmpeg[.exe] -> root is parent of bin
    let bin = path.parent()?;
    let root = bin.parent()?;
    Some(root.to_path_buf())
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
    let out = Command::new(if env::consts::OS == "windows" {
        "where"
    } else {
        "which"
    })
    .arg(exe)
    .output()
    .expect("Failed to run which/where");
    let first_line = std::str::from_utf8(&out.stdout)
        .expect("ffmpeg path not utf-8")
        .lines()
        .next()
        .expect("ffmpeg not found on PATH")
        .trim();
    PathBuf::from(first_line)
}

fn check_ffmpeg_version(ffmpeg_bin: &Path) {
    let out = Command::new(ffmpeg_bin)
        .arg("-version")
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {}: {}", ffmpeg_bin.display(), e));
    // ffmpeg often prints version to stderr on Windows
    let line_stdout = std::str::from_utf8(&out.stdout)
        .ok()
        .and_then(|s| s.lines().next())
        .unwrap_or("");
    let line_stderr = std::str::from_utf8(&out.stderr)
        .ok()
        .and_then(|s| s.lines().next())
        .unwrap_or("");
    let line = if !line_stdout.is_empty() {
        line_stdout
    } else {
        line_stderr
    };
    // "ffmpeg version 8.0" or "ffmpeg version 8.0.1" or "FFmpeg version ..."
    let version = line
        .strip_prefix("ffmpeg version ")
        .or_else(|| line.strip_prefix("FFmpeg version "))
        .or_else(|| line.split_whitespace().nth(2))
        .unwrap_or("");
    let mut parts = version.split('.');
    let major: u32 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if major < 8 {
        panic!(
            "FFmpeg 8.0 or newer is required (detected: {}). Install FFmpeg 8.0+ with --enable-libaribcaption (see README).",
            version
        );
    }
}

fn check_libaribcaption(ffmpeg_bin: &Path) {
    // ffmpeg -hide_banner -decoders lists decoders; libaribcaption shows e.g.:
    //  S..... libaribcaption       ARIB STD-B24 caption decoder (codec arib_caption)
    let out = Command::new(ffmpeg_bin)
        .args(["-hide_banner", "-decoders"])
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {} -decoders: {}", ffmpeg_bin.display(), e));
    // decoder list is often on stderr on Windows
    let out_str = std::str::from_utf8(&out.stdout).unwrap_or("");
    let out_str = if out_str.is_empty() {
        std::str::from_utf8(&out.stderr).unwrap_or("")
    } else {
        out_str
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
