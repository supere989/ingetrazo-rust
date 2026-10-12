#!/usr/bin/env bash
# Rust metadata only: use native macOS CI for SDK, C ABI, linking, MSL and GPU proof.
# These five native dependencies have checked-in Rust declarations. Cargo links
# overrides suppress only their native C build scripts; no executable is produced.
# ring is pulled in by DEM HTTPS. Native macOS jobs still build and link its
# crypto implementation; this Linux diagnostic checks Rust metadata only.
set -euo pipefail
target="${1:-aarch64-apple-darwin}"
case "$target" in
    aarch64-apple-darwin|x86_64-apple-darwin) ;;
    *) printf 'Expected an Apple target, got %s\n' "$target" >&2; exit 2 ;;
esac
host=$(rustc -vV | sed -n 's/^host: //p')
if [[ "$host" == "$target" ]]; then
    printf 'Metadata-only overrides require a cross target; use the native build on %s\n' "$host" >&2
    exit 2
fi
# Its normal build script copies this pinned, shipped file into OUT_DIR. A
# metadata override must supply the same declarations without invoking CMake.
turbojpeg_bindings=$(cargo metadata --locked --format-version=1 | python3 -c 'import json,sys; from pathlib import Path; p=[p for p in json.load(sys.stdin)["packages"] if p["name"]=="turbojpeg-sys" and p["version"]=="1.2.0"]; assert len(p)==1; root=Path(p[0]["manifest_path"]).parent; assert (root/"bindings.rs").is_file(); print(root)')
INGETRAZO_RUST_METADATA_ONLY=1 cargo check --locked -p ingetrazo-gui --target "$target" \
    --config "target.$target.z.rustc-link-lib=[\"z\"]" \
    --config "target.$target.freetype.rustc-link-lib=[\"freetype\"]" \
    --config "target.$target.harfbuzz.rustc-link-lib=[\"harfbuzz\"]" \
    --config "target.$target.turbojpeg.rustc-link-lib=[\"turbojpeg\"]" \
    --config "target.$target.turbojpeg.rustc-env.OUT_DIR=\"$turbojpeg_bindings\"" \
    --config "target.$target.ring_core_0_17_14_.rustc-link-lib=[\"static=ring_core_0_17_14_\",\"static=ring_core_0_17_14__test\"]" \
    --example sheet_image_workflows \
    --example sheet_picture_workflows \
    --example composer_picture_authoring \
    --example composer_picture_gestures \
    --example native_metal_section_fill \
    --example native_metal_billboards \
    --example native_metal_document_annotations -j2
