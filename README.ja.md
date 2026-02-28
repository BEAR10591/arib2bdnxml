# arib2bdnxml

[English](README.md)

.ts/.m2ts/.mkv/.mks ファイル内の ARIB 字幕を、libaribcaption（FFmpeg 経由）を使用してビットマップにデコードし、Blu-ray 用 PGS 字幕作成に必要な BDN XML + PNG を生成するツールです。

**対応OS**: macOS / Windows のみ

macOS では Homebrew での利用を推奨しています（`brew install bear10591/tap/arib2bdnxml`）。バイナリ配布は行っていません。

## 機能

- .ts/.m2ts/.mkv/.mks ファイルから ARIB 字幕を抽出
- libaribcaption（FFmpeg 経由）を使用してビットマップにデコード
- BDN XML + PNG を生成
- デフォルト出力 1920×1080。1280×720 → 1280×720（720p）。720×480 → 720×480（ntsc）。1440×1080 のみオプションでアナモルフィック 1440×1080
- VideoFormat 1080p、720p、1440x1080、ntsc

## 要件

- Rust（edition 2021）
- FFmpeg **8.0 以上**（libavcodec, libavformat, libavutil）で **--enable-libaribcaption** 付きのビルド
- ビルド時: clang（bindgen 用）、pkg-config

ビルド時に、FFmpeg が 8.0 以上であることと ARIB キャプションデコーダ（libaribcaption）が利用可能であることを確認します。通常の FFmpeg には **libaribcaption** が有効になっていないため、以下のいずれかで **libaribcaption 有効の FFmpeg 8.0 以上**を用意してください。

## FFmpeg の用意

FFmpeg は次の順で検出されます: **FFMPEG_DIR**（設定している場合）、**pkg-config**、または **PATH** 上の **ffmpeg**。PATH に ffmpeg がある場合（例: Windows で winget インストール後）は、`FFMPEG_DIR` を指定せずにビルドできます。

### macOS（推奨: Homebrew tap の bear10591/tap/ffmpeg）

```bash
brew install bear10591/tap/ffmpeg
```

続けて `cargo build --release` を実行してください。

参考: [bear10591/homebrew-tap](https://github.com/BEAR10591/homebrew-tap)

### Windows（Gyan.dev の FFmpeg 共有ビルド）

リンク用に **shared** ビルドを用意してください。[gyan.dev FFmpeg Builds](https://www.gyan.dev/ffmpeg/builds/) から [ffmpeg-release-full-shared.7z](https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-full-shared.7z) をダウンロードして展開してください。

展開後に `ffmpeg` が PATH に通っていれば、そのまま `cargo build --release` でビルドできます。PATH に無い場合は、インストール先のルート（`include` と `lib` がその直下にあるフォルダ）を `FFMPEG_DIR` に設定してください。

```powershell
$env:FFMPEG_DIR = "C:\path\to\ffmpeg"   # PATH に無い場合のみ
cargo build --release
```

## ビルド

```bash
cargo build --release
```

実行ファイルは `target/release/arib2bdnxml`（macOS）または `target/release/arib2bdnxml.exe`（Windows）に生成されます。

### リリース用パッケージ（FFmpeg の dylib/DLL 同梱）

ユーザーが FFmpeg を別途インストールしなくてよいよう、必要な FFmpeg ライブラリを同梱したリリース用の成果物を作るには:

```bash
export FFMPEG_DIR="$(brew --prefix ffmpeg)"              # macOS 用 FFmpeg（ビルド＋dylib コピー用）
export FFMPEG_DIR_WIN="/path/to/ffmpeg-8.0.1-full_build-shared"   # Windows 用 FFmpeg 共有ビルド（exe＋DLL 用）
./scripts/package-release.sh
```

`dist/arib2bdnxml-macos-<arch>/`（実行ファイル＋`.dylib`）と、macOS 上で実行した場合は `dist/arib2bdnxml-windows-x86_64/`（`.exe`＋`.dll`）ができます。各フォルダを ZIP して配布してください。macOS で FFmpeg が他のライブラリに依存している場合は、lib が一箇所にまとまった FFmpeg を使うか、同梱後もシステムの dylib を参照します。

## テスト

```bash
cargo test
```

FFmpeg のリンクが必要なため、上記の FFmpeg を用意した環境で実行してください。

## 使用方法

```bash
arib2bdnxml [オプション] <入力ファイル>
```

**入力形式**: .ts, .m2ts, .mkv, .mks。ARIB 字幕ストリームを含むファイルを指定してください。

### オプション

- `--anamorphic, -a`: ソースが 1440×1080 のときのみアナモルフィック出力。.mks の場合は同じ／親ディレクトリのコンパニオン .mkv から解像度を判定。詳細は「出力解像度」を参照。
- `--arib-params <オプション>`: libaribcaption オプション（key=value,key=value 形式）
  - 除外: `sub_type`（BDN/PNG 出力のため常に `bitmap`）、`ass_single_rect`（ASS 用オプションのためビットマップ出力では未使用）、`canvas_size`（出力解像度から自動設定）
  - デフォルト値: `caption_encoding=0`, `font`（後述）, `force_outline_text=0`, `ignore_background=0`, `ignore_ruby=0`, `outline_width=0.0`, `replace_drcs=0`, `replace_msz_ascii=0`, `replace_msz_japanese=0`, `replace_msz_glyph=0`。font は macOS では `"Hiragino Maru Gothic ProN, Rounded M+ 1m for ARIB"`、Windows では `"Rounded M+ 1m for ARIB"`。
- `--output, -o <ディレクトリ>`: 出力ディレクトリ（省略時は入力ファイルと同じディレクトリに `<入力ベース名>_bdnxml` を作成）
- `--debug, -d`: デバッグログを出力
- `--help, -h`: ヘルプを表示
- `--version, -v`: バージョン情報を表示

### 出力解像度

- **1280×720** → 1280×720（720p）。**720×480** → 720×480（ntsc）。**1440×1080** で `--anamorphic` 指定時 → 1440×1080。上記以外は **1920×1080**。.mks 入力時は、同じディレクトリまたは親ディレクトリのコンパニオン .mkv（.mks のベース名から `.forced` / `.jpn` / トラック番号などを除いた名前の .mkv）で解像度を判定。

### VideoFormat

BDN XML の `VideoFormat` 属性は、1920×1080 のとき `1080p`、1280×720 のとき `720p`、アナモルフィック 1440×1080 のとき `1440x1080`、720×480 のとき `ntsc` です。

### 例

```bash
# 基本的な使用（出力 1920x1080）
arib2bdnxml input.ts

# 1440x1080 ソースでアナモルフィック（1440x1080）
arib2bdnxml --anamorphic input.ts

# 出力ディレクトリを指定
arib2bdnxml --output ./output input.ts

# libaribcaption オプションを指定
arib2bdnxml --arib-params font="Hiragino Maru Gothic ProN, Rounded M+ 1m for ARIB" input.ts

# 複数のオプションを組み合わせ
arib2bdnxml -a --arib-params font="Hiragino Maru Gothic ProN, Rounded M+ 1m for ARIB" \
  --output ./output input.ts
```

### BDN XML + PNG から .sup ファイルへの変換

生成された BDN XML + PNG は [BDSup2Sub](https://github.com/mjuhasz/BDSup2Sub) と互換です。BDSup2Sub で Blu-ray 用 .sup（PGS 字幕）に変換できます。XML と PNG があるディレクトリで次を実行してください。

```bash
java -jar BDSup2Sub.jar -i -T keep -o output.sup basename.xml
```

オプション（解像度 `-r`、フレームレート `-T` など）は [BDSup2Sub のコマンドライン](https://github.com/mjuhasz/BDSup2Sub/wiki/Command-line-Interface) を参照してください。

## ライセンス

（LICENSE ファイルを参照）

## 参考

- [BDSup2Sub](https://github.com/mjuhasz/BDSup2Sub) — BDN XML + PNG から .sup への変換
- [ass2bdnxml](https://github.com/cubicibo/ass2bdnxml)
  - オリジナル: [mia-0/ass2bdnxml](https://github.com/mia-0/ass2bdnxml)
- [libaribcaption](https://github.com/xqq/libaribcaption)
- [FFmpeg](https://ffmpeg.org/)
- [gyan.dev FFmpeg Builds](https://www.gyan.dev/ffmpeg/builds/) (Windows: shared ビルド推奨)
- [bear10591/homebrew-tap](https://github.com/BEAR10591/homebrew-tap) (macOS: bear10591/tap/ffmpeg)
