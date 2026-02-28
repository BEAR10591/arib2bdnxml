#!/usr/bin/env bash
# Package release: build macOS and Windows (when on macOS) binaries and bundle
# required FFmpeg dylibs/DLLs. Run from repo root.
#
# Env:
#   FFMPEG_DIR     - macOS FFmpeg root (include + lib). Used for macOS build and dylib copy.
#   FFMPEG_DIR_WIN - Windows FFmpeg shared root (include + lib + bin). Used for cross Windows build and DLL copy.
#
# Example (macOS):
#   export FFMPEG_DIR="$(brew --prefix ffmpeg)"   # e.g. bear10591/tap/ffmpeg
#   export FFMPEG_DIR_WIN="/path/to/ffmpeg-release-full-shared"
#   ./scripts/package-release.sh

set -e
cd "$(dirname "$0")/.."
DIST=dist
rm -rf "$DIST"
mkdir -p "$DIST"

# --- macOS ---
echo "Building macOS release..."
cargo build --release
ARCH=$(uname -m)
MACOS_DIR="$DIST/arib2bdnxml-macos-$ARCH"
mkdir -p "$MACOS_DIR"
cp target/release/arib2bdnxml "$MACOS_DIR/"

if [ -n "$FFMPEG_DIR" ] && [ -d "$FFMPEG_DIR/lib" ]; then
  echo "Copying macOS FFmpeg dylibs from $FFMPEG_DIR/lib ..."
  cp -n "$FFMPEG_DIR"/lib/*.dylib "$MACOS_DIR/" 2>/dev/null || true
  # Fix install names so the bundle is self-contained (loader finds dylibs next to the binary)
  for dylib in "$MACOS_DIR"/*.dylib; do
    [ -f "$dylib" ] || continue
    name=$(basename "$dylib")
    install_name_tool -id "@executable_path/$name" "$dylib" 2>/dev/null || true
    # Fix dylib→dylib references within the copied set
    otool -L "$dylib" | tail -n +2 | awk '{print $1}' | while read -r dep; do
      depname=$(basename "$dep")
      [ -f "$MACOS_DIR/$depname" ] && install_name_tool -change "$dep" "@executable_path/$depname" "$dylib" 2>/dev/null || true
    done
  done
  # Point the main binary at local dylibs (any path ending with a copied dylib name)
  for name in "$MACOS_DIR"/*.dylib; do
    [ -f "$name" ] || continue
    name=$(basename "$name")
    otool -L "$MACOS_DIR/arib2bdnxml" | tail -n +2 | awk '{print $1}' | while read -r dep; do
      [ "$(basename "$dep")" = "$name" ] && install_name_tool -change "$dep" "@executable_path/$name" "$MACOS_DIR/arib2bdnxml" 2>/dev/null || true
    done
  done
else
  echo "FFMPEG_DIR not set or no lib/: skipping dylib copy for macOS. Set FFMPEG_DIR for bundling."
fi

# --- Windows (when host is macOS) ---
if [ "$(uname -s)" = "Darwin" ]; then
  if [ -z "$FFMPEG_DIR_WIN" ] || [ ! -d "$FFMPEG_DIR_WIN/bin" ]; then
    echo "FFMPEG_DIR_WIN not set or no bin/: skipping Windows build. Set FFMPEG_DIR_WIN to Windows FFmpeg shared root."
  else
    echo "Building Windows release..."
    export PATH="${HOME}/.rustup/toolchains/stable-$(uname -m)-apple-darwin/bin:$PATH"
    export FFMPEG_DIR="$FFMPEG_DIR_WIN"
    cargo build --release --target x86_64-pc-windows-gnu

    WIN_DIR="$DIST/arib2bdnxml-windows-x86_64"
    mkdir -p "$WIN_DIR"
    cp target/x86_64-pc-windows-gnu/release/arib2bdnxml.exe "$WIN_DIR/"
    echo "Copying Windows FFmpeg DLLs from $FFMPEG_DIR_WIN/bin ..."
    cp -n "$FFMPEG_DIR_WIN"/bin/*.dll "$WIN_DIR/" 2>/dev/null || true
    # GPL notice for distribution that includes FFmpeg
    cp "$(dirname "$0")/windows-dist-NOTICE.txt" "$WIN_DIR/NOTICE.txt"
  fi
fi

echo "Done. Artifacts under $DIST/"
