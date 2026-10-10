#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
VERSION="0.3.15"
ARCH="x86_64"
DIST="$ROOT/dist"
BUILD="$ROOT/build"
APPDIR="$BUILD/IngeTrazo.AppDir"
TARBALL="$DIST/IngeTrazo-$VERSION-linux-$ARCH.tar.gz"
APPIMAGE="$DIST/IngeTrazo-$VERSION-$ARCH.AppImage"

# appimagetool needs file even when running without FUSE.
if ! command -v file >/dev/null 2>&1; then
    echo "Error: AppImage packaging requires the file utility in the build environment." >&2
    echo "Install the file package in your devbox, then rerun package.sh." >&2
    exit 1
fi

echo "==> Building IngeTrazo $VERSION (Rust Native)..."
cd "$ROOT"
cargo build --locked --release -p ingetrazo-cli -p ingetrazo-render --bin ingetrazo --bin ingetrazo-render-worker

BIN="$ROOT/target/release/ingetrazo"
WORKER_BIN="$ROOT/target/release/ingetrazo-render-worker"
if [[ ! -f "$BIN" || ! -f "$WORKER_BIN" ]]; then
    echo "Error: Binary not found at $BIN"
    exit 1
fi

rm -rf "$BUILD" "$DIST"
mkdir -p "$BUILD" "$DIST"

echo "==> Packaging portable tarball..."
TARDIR="$BUILD/IngeTrazo-$VERSION"
mkdir -p "$TARDIR"
cp "$BIN" "$TARDIR/ingetrazo"
cp "$WORKER_BIN" "$TARDIR/ingetrazo-render-worker"
cp "$ROOT/ingetrazo/resources/icons/ingetrazo_256.png" "$TARDIR/ingetrazo.png"
cp "$ROOT/ingetrazo/packaging/ingetrazo.desktop" "$TARDIR/ingetrazo.desktop"

cat > "$TARDIR/README.txt" <<'EOF'
IngeTrazo — Native Rust 3D Modeler
==================================

Run:
  ./ingetrazo              (launch interactive GUI)
  ./ingetrazo model.igz    (open document)
  ./ingetrazo --check      (run health diagnostics)
  ./ingetrazo convert in.igz out.obj (headless conversion)

Install to user applications:
  mkdir -p ~/.local/bin ~/.local/share/applications ~/.local/share/icons/hicolor/256x256/apps
  cp ingetrazo ingetrazo-render-worker ~/.local/bin/
  cp ingetrazo.png ~/.local/share/icons/hicolor/256x256/apps/ingetrazo.png
  cp ingetrazo.desktop ~/.local/share/applications/
EOF

tar -C "$BUILD" -czf "$TARBALL" "IngeTrazo-$VERSION"
echo "Created: $TARBALL ($(du -h "$TARBALL" | cut -f1))"

echo "==> Packaging AppDir..."
mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/256x256/apps"

cp "$BIN" "$APPDIR/usr/bin/ingetrazo"
cp "$WORKER_BIN" "$APPDIR/usr/bin/ingetrazo-render-worker"
cp "$ROOT/ingetrazo/resources/icons/ingetrazo_256.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/ingetrazo.png"
cp "$ROOT/ingetrazo/resources/icons/ingetrazo_256.png" "$APPDIR/ingetrazo.png"

# Setup desktop file
sed 's|^Exec=.*|Exec=ingetrazo %f|; s|^Icon=.*|Icon=ingetrazo|' \
    "$ROOT/ingetrazo/packaging/ingetrazo.desktop" > "$APPDIR/usr/share/applications/ingetrazo.desktop"
cp "$APPDIR/usr/share/applications/ingetrazo.desktop" "$APPDIR/ingetrazo.desktop"

# Setup AppRun
cat > "$APPDIR/AppRun" <<'SH'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export PATH="$HERE/usr/bin:$PATH"
exec "$HERE/usr/bin/ingetrazo" "$@"
SH
chmod +x "$APPDIR/AppRun"

echo "==> Generating AppImage..."
# Check for appimagetool or download standalone tool
APPIMAGETOOL="$BUILD/appimagetool"
if ! command -v appimagetool &>/dev/null; then
    if [[ ! -f "$APPIMAGETOOL" ]]; then
        echo "Fetching appimagetool..."
        curl -fsSL -o "$APPIMAGETOOL" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" || \
        curl -fsSL -o "$APPIMAGETOOL" "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage" || true
        chmod +x "$APPIMAGETOOL" || true
    fi
fi

if command -v appimagetool &>/dev/null; then
    appimagetool "$APPDIR" "$APPIMAGE"
elif [[ -x "$APPIMAGETOOL" ]]; then
    ARCH=x86_64 "$APPIMAGETOOL" --appimage-extract-and-run "$APPDIR" "$APPIMAGE" || \
    ARCH=x86_64 "$APPIMAGETOOL" "$APPDIR" "$APPIMAGE"
else
    echo "Notice: appimagetool could not be fetched; AppDir is ready at $APPDIR"
fi

if [[ -f "$APPIMAGE" ]]; then
    echo "Created: $APPIMAGE ($(du -h "$APPIMAGE" | cut -f1))"
fi

echo "==> Done! Artifacts located in $DIST"
