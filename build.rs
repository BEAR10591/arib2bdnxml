// Detect FFmpeg via pkg-config or FFMPEG_DIR.
// - macOS: expect Homebrew tap ffmpeg-ursus (libaribcaption enabled).
// - Windows: expect Gyan.dev FFmpeg full build (with dev headers/libs).
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");

    let include_paths: Vec<PathBuf> = if let Ok(dir) = env::var("FFMPEG_DIR") {
        let mut inc = PathBuf::from(&dir);
        inc.push("include");
        vec![inc]
    } else {
        let mut incs = Vec::new();
        for lib in &["libavcodec", "libavformat", "libavutil"] {
            let lib = pkg_config::Config::new()
                .atleast_version(match *lib {
                    "libavcodec" | "libavformat" => "58.0.0",
                    "libavutil" => "56.0.0",
                    _ => "0.0.0",
                })
                .probe(lib)
                .unwrap_or_else(|_| panic!("{} not found. Install FFmpeg with libaribcaption (see README).", lib));
            incs.extend(lib.include_paths);
        }
        incs.sort();
        incs.dedup();
        incs
    };

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
