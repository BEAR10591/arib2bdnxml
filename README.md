# arib2bdnxml

[日本語](README.ja.md)

Extracts ARIB subtitles from .ts/.m2ts/.mkv/.mks files, decodes them to bitmap via libaribcaption (through FFmpeg), and generates BDN XML + PNG for Blu-ray PGS subtitle authoring.

**Supported platforms**: macOS / Windows only

## Features

- Extract ARIB subtitles from .ts/.m2ts/.mkv/.mks files
- Decode to bitmap using libaribcaption (via FFmpeg)
- Generate BDN XML + PNG
- Default output 1920×1080; 1280×720 → 1280×720 (720p); 720×480 → 720×480 (ntsc); optional anamorphic 1440×1080 for 1440×1080 source only
- VideoFormat 1080p, 720p, 1440x1080, or ntsc

## Requirements

- Rust (edition 2021)
- FFmpeg (libavcodec, libavformat, libavutil) with **libaribcaption** enabled
- Build time: clang (for bindgen), pkg-config

Standard FFmpeg builds do **not** enable **libaribcaption**. Use one of the following to obtain an FFmpeg build with libaribcaption.

## Setting up FFmpeg

### macOS (Homebrew tap: ffmpeg-ursus)

```bash
brew install bear10591/tap/ffmpeg-ursus
```

ffmpeg-ursus is **keg-only**, so it is not on the default PATH or visible to pkg-config. Set `FFMPEG_DIR` when building:

```bash
export FFMPEG_DIR="$(brew --prefix ffmpeg-ursus)"
cargo build --release
```

See: [bear10591/homebrew-tap](https://github.com/BEAR10591/homebrew-tap)

### Windows (Gyan.dev FFmpeg full build)

Download the **full build** (with include/lib for development) from [gyan.dev FFmpeg Builds](https://www.gyan.dev/ffmpeg/builds/) and extract it.

- Example: extract `ffmpeg-release-full.7z` to get a folder like `ffmpeg-x.x.x-full_build`.
- Set that folder as `FFMPEG_DIR` when building (`include` and `lib` must be directly under it).

```powershell
$env:FFMPEG_DIR = "C:\path\to\ffmpeg-x.x.x-full_build"
cargo build --release
```

The “bin only” build does not include development files; the **full** build is required.

## Build

```bash
cargo build --release
```

The executable is produced at `target/release/arib2bdnxml` (macOS) or `target/release/arib2bdnxml.exe` (Windows).

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

- `--anamorphic, -a`: Use anamorphic output only when source is 1440x1080 (→ 1440x1080). 1280×720 → 1280×720 (720p); 720×480 → 720×480 (ntsc); other sources get 1920x1080. For .mks (no video stream), a companion .mkv is looked up in the same or parent directory. The .mkv name is derived from the .mks stem by stripping suffixes (e.g. `.forced`, `.jpn`, `.01`), so e.g. `MOVIE.jpn.mks` or `MOVIE.01.jpn.forced.mks` will match `MOVIE.mkv`. If the companion’s video is 1440×1080, anamorphic is applied (1440x1080); if 1280×720, output is 1280×720 (720p); if 720×480, output is 720×480 (ntsc); otherwise 1920x1080.
- `--arib-params <options>`: libaribcaption options (key=value,key=value)
  - Excluded: `sub_type` (fixed to `bitmap` for BDN/PNG output), `ass_single_rect` (ASS-only option; not used for bitmap), `canvas_size` (set automatically from output resolution)
  - Defaults: `caption_encoding=0`, `font` (see below), `force_outline_text=0`, `ignore_background=0`, `ignore_ruby=0`, `outline_width=0.0`, `replace_drcs=0`, `replace_msz_ascii=0`, `replace_msz_japanese=0`, `replace_msz_glyph=0`. Font: on macOS `"Hiragino Maru Gothic ProN, Rounded M+ 1m for ARIB"`, on Windows `"Rounded M+ 1m for ARIB"`.
- `--output, -o <directory>`: Output directory (default: `<input basename>_bdnxml` next to the input file)
- `--debug, -d`: Enable debug logging
- `--help, -h`: Show help
- `--version, -v`: Show version

### Output resolution

- **1280×720** → **1280×720** (720p). **720×480** → **720×480** (ntsc). **1440×1080** with `--anamorphic` → 1440×1080. Otherwise **1920×1080**. For .mks input, a companion .mkv (same or parent directory) is used to detect resolution if present.

### VideoFormat

The BDN XML `VideoFormat` attribute is `1080p` (1920×1080), `720p` (1280×720), `1440x1080` (anamorphic), or `ntsc` (720×480).

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
java -jar BDSup2Sub.jar -o output.sup basename.xml
```

See [BDSup2Sub Command-line Interface](https://github.com/mjuhasz/BDSup2Sub/wiki/Command-line-Interface) for options (e.g. `-r` for resolution, `-T` for frame rate).

## License

See the LICENSE file.

## References

- [BDSup2Sub](https://github.com/mjuhasz/BDSup2Sub) — BDN XML + PNG to .sup
- [ass2bdnxml](https://github.com/cubicibo/ass2bdnxml) — original: [mia-0/ass2bdnxml](https://github.com/mia-0/ass2bdnxml)
- [libaribcaption](https://github.com/xqq/libaribcaption)
- [FFmpeg](https://ffmpeg.org/)
- [gyan.dev FFmpeg Builds](https://www.gyan.dev/ffmpeg/builds/) (Windows: full build recommended)
- [bear10591/homebrew-tap](https://github.com/BEAR10591/homebrew-tap) (macOS: ffmpeg-ursus)
