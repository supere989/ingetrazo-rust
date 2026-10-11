#!/usr/bin/env bash
# Rust metadata only: use native macOS CI for SDK, C ABI, linking, MSL and GPU proof.
# These three sys crates have checked-in Rust bindings. Cargo links overrides
# suppress only their native C build scripts; no output executable is produced.
set -euo pipefail
target="${1:-aarch64-apple-darwin}"
case "$target" in
    aarch64-apple-darwin|x86_64-apple-darwin) ;;
    *) printf 'Expected an Apple target, got %s\n' "$target" >&2; exit 2 ;;
esac
cargo check --locked -p ingetrazo-gui --target "$target" \
    --config "target.$target.z.rustc-link-lib=[\"z\"]" \
    --config "target.$target.freetype.rustc-link-lib=[\"freetype\"]" \
    --config "target.$target.harfbuzz.rustc-link-lib=[\"harfbuzz\"]" \
    --example native_metal_section_fill \
    --example native_metal_billboards \
    --example native_metal_document_annotations -j2
