# arib2bdnxml

.ts/.m2ts ファイル内の ARIB 字幕を、libaribcaption を使用してビットマップにデコードし、Blu-ray 用 PGS 字幕作成に必要な BDN XML + PNG を生成するツールです。

**注意**: 本プロジェクトのコードは100%生成AIによって作成されたものです。

## 機能

- .ts/.m2ts ファイルから ARIB 字幕を抽出
- libaribcaption（FFmpeg 経由）を使用してビットマップにデコード
- BDN XML + PNG を生成
- ffmpeg でカットした動画用のタイムスタンプ調整（`--ss`, `--to` オプション）
- VideoFormat の自動判定（1080p, 1080i, 720p, 480p, 480i）

## 要件

- C++17 対応コンパイラ
- Meson ビルドシステム
- FFmpeg（libavcodec, libavformat, libavutil, libswscale）
- libaribcaption（FFmpeg に統合されたデコーダー）
- libpng

## ビルド

### 依存関係のインストール

**重要**: 通常のパッケージマネージャーからインストールされる標準のFFmpegには`libaribcaption`が有効になっていません。以下の方法で`libaribcaption`が有効なFFmpegを入手してください。

#### Windows

**ビルド済みFFmpegを使用**（推奨）

1. [gyan.dev FFmpeg Builds](https://www.gyan.dev/ffmpeg/builds/) または [BtbN FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds) からFFmpegをダウンロード
2. ダウンロードしたFFmpegの`bin`ディレクトリをPATHに追加

その他の依存関係（[MSYS2](https://www.msys2.org/)を使用する場合）：

```bash
pacman -S mingw-w64-x86_64-gcc mingw-w64-x86_64-meson \
    mingw-w64-x86_64-libpng
```

#### Linux

**ビルド済みFFmpegを使用**（推奨）

1. [BtbN FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds) からFFmpegをダウンロード
2. ダウンロードしたFFmpegの`bin`ディレクトリをPATHに追加

その他の依存関係：

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install -y build-essential meson libpng-dev
```

**Fedora/RHEL:**
```bash
sudo dnf install -y gcc-c++ meson libpng-devel
```

#### macOS

**Homebrew tapを使用**（推奨）

```bash
brew install bear10591/tap/ffmpeg meson libpng
```

参考: [bear10591/homebrew-tap](https://github.com/BEAR10591/homebrew-tap)

### ビルド手順

```bash
meson setup build
meson compile -C build
```

ビルドされた実行ファイルは `build/arib2bdnxml`（Linux/macOS）または `build/arib2bdnxml.exe`（Windows）に生成されます。

## 使用方法

```bash
arib2bdnxml [オプション] <入力ファイル>
```

### オプション

- `--resolution, -r <解像度>`: 出力解像度（1920x1080, 1440x1080, 1280x720, 720x480）
  - 指定がない場合は動画解像度に基づいて自動決定されます
  - 動画解像度が 1920x1080 または 1440x1080 の場合 → 1920x1080
  - 動画解像度が 1280x720 の場合 → 1280x720
  - 動画解像度が 720x480 の場合 → 720x480
  - それ以外の解像度の場合はエラーで中断されます
- `--ss <時刻>`: タイムスタンプ調整用の開始時刻（秒数または HH:MM:SS.mmm 形式）
  - ffmpeg の `-ss` オプションでカットした動画用
  - 指定した時刻より前の字幕をスキップし、タイムコードを 00:00:00.000 から開始するように調整
  - ミリ秒まで対応（例: `--ss 300.5` または `--ss 00:05:00.500`）
- `--to <時刻>`: タイムスタンプ調整用の終了時刻（秒数または HH:MM:SS.mmm 形式）
  - ffmpeg の `-to` オプションでカットした動画用
  - 指定した時刻以降の字幕をスキップし、終了時刻を制限
  - ミリ秒まで対応（例: `--to 3300.5` または `--to 00:55:00.500`）
- `--libaribcaption-opt <オプション>`: libaribcaption オプション（key=value,key=value 形式）
  - 除外: `sub_type`, `ass_single_rect`, `canvas_size`
  - `canvas_size` は `--resolution` オプションで指定してください
  - デフォルト値: `outline_width=0.0`, `replace_msz_ascii=0`, `replace_msz_japanese=0`, `replace_drcs=0`
- `--output <ディレクトリ>`: 出力ディレクトリ（省略時は入力ファイルと同じディレクトリに`<動画ファイル名>_bdnxml`を作成）
- `--debug`: デバッグログを出力
- `--help, -h`: ヘルプを表示
- `--version, -v`: バージョン情報を表示

### VideoFormat の自動判定

生成される BDN XML の `VideoFormat` 属性は、以下のルールで自動判定されます：

- **canvas_size の縦解像度**と**入力.tsファイルのインターレース判定**に基づいて決定
- 1080 ライン: インターレース → `1080i`、プログレッシブ → `1080p`
- 720 ライン: 常に `720p`（BDMV 仕様上 720i は存在しない）
- 480 ライン: インターレース → `480i`、プログレッシブ → `480p`

### 例

```bash
# 基本的な使用
arib2bdnxml input.ts

# 解像度を指定
arib2bdnxml --resolution 1920x1080 input.ts

# 出力ディレクトリを指定
arib2bdnxml --output ./output input.ts

# libaribcaption オプションを指定
arib2bdnxml --libaribcaption-opt font="Hiragino Maru Gothic ProN, Rounded M+ 1m for ARIB" input.ts

# ffmpeg でカットした動画用（00:05:00.500 から 00:55:00.500 まで）
arib2bdnxml --ss 00:05:00.500 --to 00:55:00.500 input.ts

# 秒数で指定（300.5 秒から 3300.5 秒まで）
arib2bdnxml --ss 300.5 --to 3300.5 input.ts

# 複数のオプションを組み合わせ
arib2bdnxml --resolution 1440x1080 --ss 00:00:09.871 --to 00:20:09.870 \
  --libaribcaption-opt font="Hiragino Maru Gothic ProN, Rounded M+ 1m for ARIB" \
  --output ./output input.ts
```

### BDN XML + PNG から .sup ファイルへの変換

生成された BDN XML + PNG ファイルは、[SUPer](https://github.com/quietvoid/super) を使用して Blu-ray 用の .sup ファイル（PGS 字幕）に変換できます。

## ライセンス

（LICENSE ファイルを参照）

## 参考

- [ass2bdnxml](https://github.com/cubicibo/ass2bdnxml)
  - オリジナル: [mia-0/ass2bdnxml](https://github.com/mia-0/ass2bdnxml)
- [libaribcaption](https://github.com/xqq/libaribcaption)
- [FFmpeg](https://ffmpeg.org/)
- [gyan.dev FFmpeg Builds](https://www.gyan.dev/ffmpeg/builds/) (Windows)
- [BtbN FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds) (Windows/Linux)
- [bear10591/homebrew-tap](https://github.com/BEAR10591/homebrew-tap) (macOS)
