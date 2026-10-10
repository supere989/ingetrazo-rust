#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BIN="$ROOT/target/release/ingetrazo"

if [[ ! -f "$BIN" ]]; then
    echo "==> Building release binary first..."
    cargo build --release -p ingetrazo-cli
fi

BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/256x256/apps"

mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR"

echo "==> Installing binary to $BIN_DIR/ingetrazo..."
cp "$BIN" "$BIN_DIR/ingetrazo"
chmod +x "$BIN_DIR/ingetrazo"

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
