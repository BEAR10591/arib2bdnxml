# arib2bdnxml

[日本語](README.ja.md)

Extracts ARIB subtitles from .ts/.m2ts files, decodes them to bitmap via libaribcaption (through FFmpeg), and generates BDN XML + PNG for Blu-ray PGS subtitle authoring.

**Supported platforms**: macOS / Windows only

## Features

- Extract ARIB subtitles from .ts/.m2ts files
- Decode to bitmap using libaribcaption (via FFmpeg)
- Generate BDN XML + PNG
- Timestamp adjustment for footage cut with ffmpeg (`--ss`, `--to` options)
- Automatic VideoFormat detection (1080p, 1080i, 720p, 480p, 480i)

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

### Options

- `--resolution, -r <resolution>`: Output resolution (1920x1080, 1440x1080, 1280x720, 720x480)
  - If omitted, determined from the video resolution
  - 1920x1080 or 1440x1080 → 1920x1080
  - 1280x720 → 1280x720
  - 720x480 → 720x480
  - Other resolutions cause an error
- `--ss <time>`: Start time for timestamp adjustment (seconds or HH:MM:SS.mmm)
  - For video cut with ffmpeg’s `-ss`
  - Skips subtitles before this time and adjusts timecodes to start at 00:00:00.000
  - Supports milliseconds (e.g. `--ss 300.5` or `--ss 00:05:00.500`)
- `--to <time>`: End time for timestamp adjustment (seconds or HH:MM:SS.mmm)
  - For video cut with ffmpeg’s `-to`
  - Skips subtitles at or after this time
  - Supports milliseconds (e.g. `--to 3300.5` or `--to 00:55:00.500`)
- `--libaribcaption-opt <options>`: libaribcaption options (key=value,key=value)
  - Excluded: `sub_type`, `ass_single_rect`, `canvas_size`
  - Use `--resolution` for `canvas_size`
  - Defaults: `outline_width=0.0`, `replace_msz_ascii=0`, `replace_msz_japanese=0`, `replace_drcs=0`
- `--output <directory>`: Output directory (default: `<input basename>_bdnxml` next to the input file)
- `--debug`: Enable debug logging
- `--help, -h`: Show help
- `--version, -v`: Show version

### VideoFormat auto-detection

The BDN XML `VideoFormat` attribute is determined as follows:

- Based on **canvas height** and **interlaced flag** from the input .ts
- 1080 lines: interlaced → `1080i`, progressive → `1080p`
- 720 lines: always `720p` (no 720i in BDMV spec)
- 480 lines: interlaced → `480i`, progressive → `480p`

### Examples

```bash
# Basic usage
arib2bdnxml input.ts

# Specify resolution
arib2bdnxml --resolution 1920x1080 input.ts

# Specify output directory
arib2bdnxml --output ./output input.ts

# libaribcaption options
arib2bdnxml --libaribcaption-opt font="Hiragino Maru Gothic ProN, Rounded M+ 1m for ARIB" input.ts

# For video cut from 00:05:00.500 to 00:55:00.500
arib2bdnxml --ss 00:05:00.500 --to 00:55:00.500 input.ts

# Time in seconds (300.5 to 3300.5)
arib2bdnxml --ss 300.5 --to 3300.5 input.ts

# Combined options
arib2bdnxml --resolution 1440x1080 --ss 00:00:09.871 --to 00:20:09.870 \
  --libaribcaption-opt font="Hiragino Maru Gothic ProN, Rounded M+ 1m for ARIB" \
  --output ./output input.ts
```

### BDN XML + PNG to .sup

Use [SUPer](https://github.com/quietvoid/super) to convert the generated BDN XML + PNG into Blu-ray .sup (PGS) subtitle files.

## License

See the LICENSE file.

## References

- [ass2bdnxml](https://github.com/cubicibo/ass2bdnxml) — original: [mia-0/ass2bdnxml](https://github.com/mia-0/ass2bdnxml)
- [libaribcaption](https://github.com/xqq/libaribcaption)
- [FFmpeg](https://ffmpeg.org/)
- [gyan.dev FFmpeg Builds](https://www.gyan.dev/ffmpeg/builds/) (Windows: full build recommended)
- [bear10591/homebrew-tap](https://github.com/BEAR10591/homebrew-tap) (macOS: ffmpeg-ursus)
