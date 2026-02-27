// Detect FFmpeg via FFMPEG_DIR, pkg-config, or PATH (ffmpeg executable).
// - macOS: expect Homebrew tap bear10591/tap/ffmpeg (libaribcaption enabled).
// - Windows: expect Gyan.dev FFmpeg full build (with dev headers/libs); winget or PATH.
// Requires FFmpeg 8.0+ built with --enable-libaribcaption.
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// LIBAVCODEC_VERSION_MAJOR for FFmpeg 8.0 (we require 8.0+ for libaribcaption).
const FFMPEG_8_MAJOR: u32 = 62;

/// Read LIBAVCODEC_VERSION_MAJOR from libavcodec/version_major.h in the given include paths.
fn version_from_headers(include_paths: &[PathBuf]) -> Option<u32> {
    for inc in include_paths {
        let path = inc.join("libavcodec").join("version_major.h");
        let s = std::fs::read_to_string(&path).ok()?;
        for line in s.lines() {
            let line = line.trim();
            if !line.starts_with("#define") || !line.contains("LIBAVCODEC_VERSION_MAJOR") {
                continue;
            }
            let rest = line.strip_prefix("#define")?.trim();
            let rest = rest.strip_prefix("LIBAVCODEC_VERSION_MAJOR")?.trim();
            let num_str = rest.split_whitespace().next()?;
            if let Ok(n) = num_str.parse::<u32>() {
                return Some(n);
            }
        }
    }
    None
}

fn lib_patterns_for_os() -> &'static [&'static str] {
    if env::consts::OS == "windows" {
        &["avcodec.dll", "avcodec.lib", "libavcodec.dll.a", "libavcodec.a"]
    } else if env::consts::OS == "macos" {
        &["libavcodec.a", "libavcodec.dylib"]
    } else {
        &["libavcodec.a", "libavcodec.so"]
    }
}

/// Check for libaribcaption by looking for ff_libaribcaption_decoder in libavcodec.
/// On Windows, also looks in root/bin/avcodec.dll when root is set.
/// Prefer static .a when present (e.g. Homebrew dylib may not export the symbol).
fn check_libaribcaption_via_lib(link_search: &Option<PathBuf>, root: &Option<PathBuf>) -> bool {
    const SYMBOL: &str = "ff_libaribcaption_decoder";
    let lib_patterns = lib_patterns_for_os();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(lib_dir) = link_search {
        if lib_dir.exists() {
            for name in lib_patterns {
                let p = lib_dir.join(name);
                if p.exists() {
                    candidates.push(p);
                }
            }
        }
    }
    if env::consts::OS == "windows" {
        if let Some(r) = root {
            let dll = r.join("bin").join("avcodec.dll");
            if dll.exists() {
                candidates.push(dll);
            }
        }
    }
    let lib_path = match candidates.into_iter().next() {
        Some(p) => p,
        None => return false,
    };
    if env::consts::OS == "windows" {
        let ext = lib_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "dll" {
            if let Some(found) = pe_dll_exports_contain(&lib_path, SYMBOL) {
                return found;
            }
        }
    }
    let (cmd, args): (&str, &[&str]) = if env::consts::OS == "windows" {
        let ext = lib_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "a" {
            ("nm", &["-g", "-C"])
        } else {
            ("dumpbin", &["/EXPORTS"])
        }
    } else if env::consts::OS == "macos" {
        ("nm", &["-g", "-U"])
    } else {
        ("nm", &["-g"])
    };
    let out = match Command::new(cmd).args(args).arg(&lib_path).output() {
        Ok(o) => o,
        Err(_) => return false,
    };
    let out_str = String::from_utf8_lossy(&out.stdout);
    let err_str = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{}\n{}", out_str, err_str);
    combined.contains(SYMBOL)
}

/// Check if a Windows DLL exports a symbol by reading the PE export table (no dumpbin needed).
/// Returns Some(true) if symbol is found, Some(false) if not, None if the file is not a valid PE or parse failed.
#[cfg(target_os = "windows")]
fn pe_dll_exports_contain(path: &Path, symbol: &str) -> Option<bool> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    if buf.len() < 64 {
        return None;
    }
    let e_lfanew = u32::from_le_bytes([buf[0x3C], buf[0x3D], buf[0x3E], buf[0x3F]]) as usize;
    if e_lfanew + 24 > buf.len() || buf[e_lfanew..].get(0..4) != Some(b"PE\0\0") {
        return None;
    }
    let coff = e_lfanew + 4;
    let num_sections = u16::from_le_bytes([buf[coff + 2], buf[coff + 3]]) as usize;
    let size_of_optional = u16::from_le_bytes([buf[coff + 16], buf[coff + 17]]) as usize;
    let opt = coff + 20;
    let export_rva = if size_of_optional >= 128 {
        let dd = opt + size_of_optional - 128;
        u32::from_le_bytes([buf[dd], buf[dd + 1], buf[dd + 2], buf[dd + 3]])
    } else {
        return None;
    };
    let section_table = opt + size_of_optional;
    let section_size = 40;
    let mut export_file_offset = None;
    for i in 0..num_sections {
        let sec = section_table + i * section_size;
        if sec + 40 > buf.len() {
            break;
        }
        let va = u32::from_le_bytes([buf[sec + 12], buf[sec + 13], buf[sec + 14], buf[sec + 15]]);
        let raw_size = u32::from_le_bytes([buf[sec + 16], buf[sec + 17], buf[sec + 18], buf[sec + 19]]);
        let raw_ptr = u32::from_le_bytes([buf[sec + 20], buf[sec + 21], buf[sec + 22], buf[sec + 23]]);
        if export_rva >= va && export_rva < va.saturating_add(raw_size) {
            export_file_offset = Some((raw_ptr + (export_rva - va)) as usize);
            break;
        }
    }
    let export_off = export_file_offset?;
    if export_off + 40 > buf.len() {
        return None;
    }
    let num_names = u32::from_le_bytes([
        buf[export_off + 24],
        buf[export_off + 25],
        buf[export_off + 26],
        buf[export_off + 27],
    ]) as usize;
    let addr_of_names = u32::from_le_bytes([
        buf[export_off + 32],
        buf[export_off + 33],
        buf[export_off + 34],
        buf[export_off + 35],
    ]);
    let rva_to_offset = |rva: u32| -> Option<usize> {
        for i in 0..num_sections {
            let sec = section_table + i * section_size;
            if sec + 40 > buf.len() {
                break;
            }
            let va = u32::from_le_bytes([buf[sec + 12], buf[sec + 13], buf[sec + 14], buf[sec + 15]]);
            let raw_size = u32::from_le_bytes([buf[sec + 16], buf[sec + 17], buf[sec + 18], buf[sec + 19]]);
            let raw_ptr = u32::from_le_bytes([buf[sec + 20], buf[sec + 21], buf[sec + 22], buf[sec + 23]]);
            if rva >= va && rva < va.saturating_add(raw_size) {
                return Some((raw_ptr + (rva - va)) as usize);
            }
        }
        None
    };
    let names_array_off = rva_to_offset(addr_of_names)?;
    for i in 0..num_names {
        let name_rva_off = names_array_off + i * 4;
        if name_rva_off + 4 > buf.len() {
            break;
        }
        let name_rva = u32::from_le_bytes([
            buf[name_rva_off],
            buf[name_rva_off + 1],
            buf[name_rva_off + 2],
            buf[name_rva_off + 3],
        ]);
        let name_off = rva_to_offset(name_rva)?;
        let end = buf[name_off..].iter().position(|&b| b == 0).unwrap_or(256);
        if let Ok(s) = std::str::from_utf8(&buf[name_off..name_off + end]) {
            if s == symbol {
                return Some(true);
            }
        }
    }
    Some(false)
}

#[cfg(not(target_os = "windows"))]
fn pe_dll_exports_contain(_path: &Path, _symbol: &str) -> Option<bool> {
    None
}

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

    let version_from_headers = version_from_headers(&include_paths);
    match version_from_headers {
        Some(m) if m >= FFMPEG_8_MAJOR => {}
        Some(m) => panic!(
            "FFmpeg 8.0 or newer is required (headers show LIBAVCODEC_VERSION_MAJOR = {}). \
             Install FFmpeg 8.0+ with --enable-libaribcaption (see README).",
            m
        ),
        None => panic!(
            "Could not determine FFmpeg version: libavcodec/version_major.h not found in include paths. \
             Install FFmpeg 8.0+ with development headers (see README)."
        ),
    }
    if !check_libaribcaption_via_lib(&link_search, &root) {
        panic!(
            "FFmpeg was not built with --enable-libaribcaption (ff_libaribcaption_decoder not found in lib). \
             Use an FFmpeg 8.0+ build with libaribcaption enabled (see README)."
        );
    }

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

