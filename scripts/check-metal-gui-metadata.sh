#!/usr/bin/env bash
# Rust metadata only: use native macOS CI for SDK, C ABI, linking, MSL and GPU proof.
# These four sys crates have checked-in Rust bindings. Cargo links overrides
# suppress only their native C build scripts; no output executable is produced.
set -euo pipefail
target="${1:-aarch64-apple-darwin}"
case "$target" in
    aarch64-apple-darwin|x86_64-apple-darwin) ;;
    *) printf 'Expected an Apple target, got %s\n' "$target" >&2; exit 2 ;;
esac
# Its normal build script copies this pinned, shipped file into OUT_DIR. A
# metadata override must supply the same declarations without invoking CMake.
turbojpeg_bindings=$(cargo metadata --locked --format-version=1 | python3 -c 'import json,sys; from pathlib import Path; p=[p for p in json.load(sys.stdin)["packages"] if p["name"]=="turbojpeg-sys" and p["version"]=="1.2.0"]; assert len(p)==1; root=Path(p[0]["manifest_path"]).parent; assert (root/"bindings.rs").is_file(); print(root)')
INGETRAZO_RUST_METADATA_ONLY=1 cargo check --locked -p ingetrazo-gui --target "$target" \
    --config "target.$target.z.rustc-link-lib=[\"z\"]" \
    --config "target.$target.freetype.rustc-link-lib=[\"freetype\"]" \
    --config "target.$target.harfbuzz.rustc-link-lib=[\"harfbuzz\"]" \
    --config "target.$target.turbojpeg.rustc-link-lib=[\"turbojpeg\"]" \
    --config "target.$target.turbojpeg.rustc-env.OUT_DIR=\"$turbojpeg_bindings\"" \
    --example sheet_image_workflows \
    --example native_metal_section_fill \
    --example native_metal_billboards \
    --example native_metal_document_annotations -j2
