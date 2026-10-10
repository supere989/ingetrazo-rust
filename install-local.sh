#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BIN="$ROOT/target/release/ingetrazo"
WORKER_BIN="$ROOT/target/release/ingetrazo-render-worker"

if [[ ! -f "$BIN" || ! -f "$WORKER_BIN" ]]; then
    echo "==> Building release binary first..."
    (cd "$ROOT" && cargo build --locked --release -p ingetrazo-cli -p ingetrazo-render --bin ingetrazo --bin ingetrazo-render-worker)
fi

INSTALL_PREFIX="${INGETRAZO_INSTALL_PREFIX:-${HOME}/.local}"
BIN_DIR="$INSTALL_PREFIX/bin"
APP_DIR="$INSTALL_PREFIX/share/applications"
ICON_DIR="$INSTALL_PREFIX/share/icons/hicolor/256x256/apps"

mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR"

echo "==> Installing binary to $BIN_DIR/ingetrazo..."
cp "$BIN" "$BIN_DIR/ingetrazo"
cp "$WORKER_BIN" "$BIN_DIR/ingetrazo-render-worker"
chmod +x "$BIN_DIR/ingetrazo" "$BIN_DIR/ingetrazo-render-worker"

echo "==> Installing icon to $ICON_DIR/ingetrazo.png..."
cp "$ROOT/ingetrazo/resources/icons/ingetrazo_256.png" "$ICON_DIR/ingetrazo.png"

echo "==> Installing desktop entry to $APP_DIR/ingetrazo.desktop..."
sed "s|^Exec=.*|Exec=$BIN_DIR/ingetrazo %f|; s|^Icon=.*|Icon=$ICON_DIR/ingetrazo.png|" \
    "$ROOT/ingetrazo/packaging/ingetrazo.desktop" > "$APP_DIR/ingetrazo.desktop"
chmod +x "$APP_DIR/ingetrazo.desktop"

# Update desktop database if utility exists
if command -v update-desktop-database &>/dev/null; then
    update-desktop-database "$APP_DIR" 2>/dev/null || true
fi

echo "==> Successfully installed IngeTrazo (Rust Native)!"
echo "You can launch it from your application menu or via terminal: ingetrazo"
