# arib2bdnxml

[日本語](README.ja.md)

Extracts ARIB subtitles from .ts/.m2ts/.mkv/.mks files, decodes them to bitmap via libaribcaption (through FFmpeg), and generates BDN XML + PNG for Blu-ray PGS subtitle authoring.

**Supported platforms**: macOS / Windows only

On macOS we recommend using Homebrew (`brew install bear10591/tap/arib2bdnxml`). We do not distribute binaries.

## Features

- Extract ARIB subtitles from .ts/.m2ts/.mkv/.mks files
- Decode to bitmap using libaribcaption (via FFmpeg)
- Generate BDN XML + PNG
- Default output 1920×1080; 1280×720 → 1280×720 (720p); 720×480 → 720×480 (ntsc); optional anamorphic 1440×1080 for 1440×1080 source only
- VideoFormat 1080p, 720p, 1440x1080, or ntsc

## Requirements

- Rust (edition 2021)
- FFmpeg **8.0 or newer** (libavcodec, libavformat, libavutil) built with **--enable-libaribcaption**
- Build time: clang (for bindgen), pkg-config

The build will check that the FFmpeg version is 8.0+ and that the ARIB caption decoder (libaribcaption) is available. Standard FFmpeg builds do **not** enable **libaribcaption**. Use one of the following to obtain an FFmpeg 8.0+ build with libaribcaption.

## Setting up FFmpeg

FFmpeg is detected in this order: **FFMPEG_DIR** (if set), **pkg-config**, or the **ffmpeg** executable on **PATH**. If ffmpeg is on your PATH (e.g. after `winget install` on Windows), you can build without setting `FFMPEG_DIR`.

### macOS (Homebrew tap: bear10591/tap/ffmpeg)

```bash
brew install bear10591/tap/ffmpeg
```

Then run `cargo build --release`.

See: [bear10591/homebrew-tap](https://github.com/BEAR10591/homebrew-tap)

### Windows (Gyan.dev FFmpeg shared build)

Use a **shared** build (needed for linking): download [ffmpeg-release-full-shared.7z](https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-full-shared.7z) from [gyan.dev FFmpeg Builds](https://www.gyan.dev/ffmpeg/builds/) and extract it.

If `ffmpeg` is on your PATH after extraction, run `cargo build --release` with no extra setup. Otherwise set `FFMPEG_DIR` to the installation root (the folder that contains `include` and `lib`):

```powershell
$env:FFMPEG_DIR = "C:\path\to\ffmpeg"   # only if ffmpeg is not on PATH
cargo build --release
```

## Build

```bash
cargo build --release
```

The executable is produced at `target/release/arib2bdnxml` (macOS) or `target/release/arib2bdnxml.exe` (Windows).

### Release packaging (bundle with FFmpeg dylibs/DLLs)

To build release artifacts that include the required FFmpeg libraries so users do not need to install FFmpeg:

```bash
export FFMPEG_DIR="$(brew --prefix ffmpeg)"              # macOS FFmpeg (for build + dylibs)
export FFMPEG_DIR_WIN="/path/to/ffmpeg-8.0.1-full_build-shared"   # Windows FFmpeg shared (for exe + DLLs)
./scripts/package-release.sh
```

This produces `dist/arib2bdnxml-macos-<arch>/` (executable + `.dylib`) and, on macOS host, `dist/arib2bdnxml-windows-x86_64/` (`.exe` + `.dll`). Zip each folder for distribution. On macOS, if FFmpeg has dependencies outside that lib (e.g. Homebrew), use an FFmpeg build whose libs are self-contained, or the bundle will still load system libs.

**Windows distribution and GPL:** The Windows package that bundles FFmpeg DLLs uses a GPL-licensed FFmpeg build. That combined distribution is under the GPL. The archive includes `NOTICE.txt` with the GPL notice and information on where to obtain the FFmpeg source code (e.g. [FFmpeg](https://ffmpeg.org/), [gyan.dev builds](https://www.gyan.dev/ffmpeg/builds/)).

## Tests

```bash
cargo test
```

FFmpeg must be available for linking; run tests in an environment where the FFmpeg above is set up.

## Usage

```bash
arib2bdnxml [options] <input file>
```

**Input formats**: .ts, .m2ts, .mkv, .mks. The file must contain an ARIB subtitle stream.

### Options

- `--anamorphic, -a`: Use anamorphic output only when source is 1440×1080. For .mks (no video stream), resolution is taken from a companion .mkv in the same or parent directory (see **Output resolution**).
- `--arib-params <options>`: libaribcaption options (key=value,key=value)
  - Excluded: `sub_type` (fixed to `bitmap` for BDN/PNG output), `ass_single_rect` (ASS-only option; not used for bitmap), `canvas_size` (set automatically from output resolution)
  - Defaults: `caption_encoding=0`, `font` (see below), `force_outline_text=0`, `ignore_background=0`, `ignore_ruby=0`, `outline_width=0.0`, `replace_drcs=0`, `replace_msz_ascii=0`, `replace_msz_japanese=0`, `replace_msz_glyph=0`. Font: on macOS `"Hiragino Maru Gothic ProN, Rounded M+ 1m for ARIB"`, on Windows `"Rounded M+ 1m for ARIB"`.
- `--output, -o <directory>`: Output directory (default: `<input basename>_bdnxml` next to the input file)
- `--debug, -d`: Enable debug logging
- `--help, -h`: Show help
- `--version, -v`: Show version

### Output resolution

- **1280×720** → 1280×720 (720p). **720×480** → 720×480 (ntsc). **1440×1080** with `--anamorphic` → 1440×1080. Otherwise **1920×1080**. For .mks input, a companion .mkv in the same or parent directory is used to detect resolution; the .mkv name is derived from the .mks stem by stripping suffixes (e.g. `.forced`, `.jpn`, `.01`), so e.g. `MOVIE.jpn.mks` or `MOVIE.01.jpn.forced.mks` matches `MOVIE.mkv`.

### VideoFormat

The BDN XML `VideoFormat` attribute is set from the output resolution: `1080p`, `720p`, `1440x1080` (anamorphic), or `ntsc`.

### Examples

```bash
# Basic usage (output 1920x1080)
arib2bdnxml input.ts

# Anamorphic for 1440x1080 source (1440x1080)
arib2bdnxml --anamorphic input.ts

# Specify output directory
arib2bdnxml --output ./output input.ts

# libaribcaption options
arib2bdnxml --arib-params font="Hiragino Maru Gothic ProN, Rounded M+ 1m for ARIB" input.ts

# Combined options
arib2bdnxml -a --arib-params font="Hiragino Maru Gothic ProN, Rounded M+ 1m for ARIB" \
  --output ./output input.ts
```

### BDN XML + PNG to .sup

The generated BDN XML + PNG are compatible with [BDSup2Sub](https://github.com/mjuhasz/BDSup2Sub). Use BDSup2Sub to convert them to Blu-ray .sup (PGS) subtitle files. Run BDSup2Sub from the directory that contains the XML and PNG files:

```bash
java -jar BDSup2Sub.jar -i -T keep -o output.sup basename.xml
```

See [BDSup2Sub Command-line Interface](https://github.com/mjuhasz/BDSup2Sub/wiki/Command-line-Interface) for options (e.g. `-r` for resolution, `-T` for frame rate).

## License

See the LICENSE file.

## References

- [BDSup2Sub](https://github.com/mjuhasz/BDSup2Sub) — BDN XML + PNG to .sup
- [ass2bdnxml](https://github.com/cubicibo/ass2bdnxml) — original: [mia-0/ass2bdnxml](https://github.com/mia-0/ass2bdnxml)
- [libaribcaption](https://github.com/xqq/libaribcaption)
- [FFmpeg](https://ffmpeg.org/)
- [gyan.dev FFmpeg Builds](https://www.gyan.dev/ffmpeg/builds/) (Windows: shared build recommended)
- [bear10591/homebrew-tap](https://github.com/BEAR10591/homebrew-tap) (macOS: bear10591/tap/ffmpeg)
