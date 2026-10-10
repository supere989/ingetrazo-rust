# Initial parity audit evidence

Collected 2026-10-09 in a base devbox against Rust commit 68368964fd5625f96ab5c705c43cdc13e402a6bd and Python submodule 6be29fe437a7cd992d4e079f71720821b25216b9. See [tracker](../PARITY_PROGRESS.md) for scope and limitations. No original documents were modified.

## First swarm milestone

Source tests and retained checker results distinguish source baselines from corrected implementations. `python-reopen-unfrozen-initial.json` records generated name/UID differences before the protocol froze generation separately for each load; persisted IDs are still compared and covered by negative controls. `render-startup-x11-missing.log` and `render-startup-xkb-missing.log` are isolated container dependency failures. `render-texture-panic-baseline.log` is an actual Rust rendering defect found by launching the example. The textured-vertex construction was corrected, and `render-pergola.png` was inspected: visible model, textures, annotations, scene list and application controls. This is a software-GL virtual-display startup check, not desktop GPU or full interaction parity. Reproduce with `scripts/render-smoke.sh` after building the CLI in a devbox with Xvfb, ImageMagick and the native X11/GL runtime libraries specified in CI.
