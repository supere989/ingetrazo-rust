# Scale ingress delta

Seven exact real egui input cases and four exact pinned Qt eye-clipping cases pass; six corruptions reject. The initial record retains immediate acquisition mutation and missed coalesced press. The production default is 8 px (Viewport.__init__); the Scale stub fallback 10 px is not the application setting.

Source tools/scale.py on_click/_grab records identity factors at press. Real viewport mousePressEvent delivers the press coordinates before later motion. Native now preserves those coordinates/modifiers and only updates Scale on delivered motion, including motion after a coalesced press. Existing non-Scale dispatch is unchanged. Active coalesced commit recomputes its ray/snap at the press coordinates. Box segments use the actual original Qt-positive-w clipping law; renderer clipping remains separate.

Replay inside ingetrazo-publish:

```sh
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build -j2 -p ingetrazo-gui --example scale_egui_input
/tmp/ingetrazo-metal-world-target/debug/examples/scale_egui_input /tmp/scale-egui.json docs/audits/scale-egui-ingress/source-eye-clip.json
python3 scripts/check-scale-egui-input.py docs/audits/scale-egui-ingress/source-eye-clip.json /tmp/scale-egui.json
```

Undo/Redo here invokes real app methods after actual input commit; there is no keyboard shortcut or OS/rendered claim. This does not prove arbitrary runtime threshold overrides, rapid Ctrl/Shift tap transport, every snap kind, context exit, or complete Scale rendering. Existing root model/history source records are reused without recapture. The initial binary predates base alignment and carried copied root prerequisites; only final provenance authenticates the final runtime and fixture.
