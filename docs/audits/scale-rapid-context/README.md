# Scale rapid modifiers and image context retype

Runtime `deed301f` retains native modifier transitions even when a press/release produces no egui key event. Transitions are delivered at pointer-event watermarks; frame-final state remains the compatibility path when no native timeline is installed. Two bounded retained buffers reuse their capacities; a refused batch cannot acquire/move/release Scale.

Nine pinned-original recipes compare 52 states exactly: six Ctrl/Shift single/double/mixed taps, image-only preview across one/two nested Finish boundaries, and committed image Scale→Finish→hot retype. Geometry/image coordinates, active acquisition, context depth and Undo/Redo counts are literal. Original is real QWidget key delivery plus operative Scale/context/history handlers. Native is actual retained ingress and egui update_ui plus app dispatch. This is not physical keyboard/pointer or rendered acceptance.

Finish validates before touching tool state, expires stale command windows, and rebinds only an image-only window when publication preserves the chronological head (`!fold || !changed`). An unrelated actual save_undo_state publication remains expired. Folded geometry history is not resurrected.

Initial native image retype failure remains in initial-failure.json. A same-binary final-state-only ingress counterfactual fails the strict checker. Six corpus/geometry/history corruptions also reject. Seven timeline tests cover zero-event taps, event watermarks, bounded atomic refusal/recovery and unchanged retained pointer/change buffers. The affected previous five-context/23-state checker also passes with its six explicit historical integrity divergences unchanged (affected-context-check.json).

Remaining: native timeline is installed by Vulkan/Metal hosts; default Glow has frame-final compatibility, not rapid-tap parity. Autorepeated modifier key behavior, physical macOS/XCB ingress, arbitrary interleaved pointer batches and full rendered/context lifetime remain unproved. This batch does not remove the prior mixed geometry/context source-integrity divergences. No broad suite was run.

Replay in ingetrazo-publish:

```sh
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build -p ingetrazo-gui --example scale_rapid_context -j2
/tmp/ingetrazo-metal-world-target/debug/examples/scale_rapid_context docs/audits/scale-rapid-context/source.json /tmp/scale-rapid-native.json
python3 scripts/check-scale-rapid-context.py docs/audits/scale-rapid-context
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -p ingetrazo-gui --lib pointer_modifiers::tests -j2
```

The checker above checks committed observations; replace native.json only in a copied evidence directory for a fresh replay. Source pin/provenance and literal nine-recipe inventory are mandatory.
