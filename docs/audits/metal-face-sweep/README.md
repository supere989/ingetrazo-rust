# Direct native Metal borrowed face sweep consumer

The application passes its actual Tool::face_sweep_preview to the native world route with the Qt-float camera eye. Overlay caps/walls use fixed front paint and front texture on both sides, after ordinary surfaces/edges/ghosts; prism replacements use ordinary materials and layers. Ordered endpoint lines use the original GL_LINES width1 contract, compact12-byte positions, and the existing world render pass. No authoritative Scene mutation, managed image wrapper, extra render process or readback is used in production.

The World retains at most the original and suppressed base meshes for the active acquisition. Immutable offset geometry changes upload the preview prototype and its endpoint buffer; unchanged source prototypes reuse existing native buffers. Camera-side suppression reuses cap/wall GPU keys. Cancel finds the original cached geometry. All candidates are private until complete admission. Prepared owners retain matching buffers/images/pipelines/targets; shared budget charges cover extra base variants. base_variant_gpu_bytes reports extra buffer residency excluding current placements and shared images. CPU descriptor traversal and material ordering are still O(prototypes/operations); this is not a globally O(changed) renderer. CPU preview rebuilding on a suppression-side change remains O(affected geometry).

Focused host cache test passes1/1 (offset variant retention, camera-side GPU key reuse, invalid acquisition and unchanged original packet). ARM Rust library/actual app diagnostic type checking passes. Initial GUI crosscheck lacked a Mac zlib toolchain; final Rust check used host C with default target C flags disabled solely to unblock cargo check. This does not establish Mac linking or shader compilation. No actual Metal preview execution or strict pixels are claimed yet.

The new native_metal_face_sweep executable consumes the authenticated six-source-case archive: actual app click/hover acquisition, base/preview/offset/Cancel, immutable model/history, repeated-frame0geometry/line uploads, offset-only1prototype/no base-line/no image uploads, admission pressure preserving old pixels/charges and retry, cached Cancel, final ledger0. These are assertions awaiting real Apple execution. The checker retains strict24wholeRGBA comparisons and five malformed/pixel negatives. Source distance-label pixels are not masked: the scene-FBO readback lacks their annotation consumer, so that complete gate remains open alongside previously quantified vendor MSAA boundaries.

Existing section/shadow/image/annotation/profile/grid and richer edge modes retain explicit admission boundaries. Production UI/OS gesture, all preview styles and full arbitrary geometry parity remain required beyond this bounded corpus. The source oracle is reused from docs/audits/face-sweep-source; no source recapture.

Apple workflow restores both runners and builds four named consumers in one command; the existing classic/material/ghost controls run alongside this shader change. Execute:

    python3 scripts/check-metal-face-sweep.py docs/audits/face-sweep-source "$RUNNER_TEMP/metal-sweep-input" --unpack
    cargo build --locked -p ingetrazo-gui --example native_metal_face_sweep --example native_metal_hidden_ghost --example native_metal_world_classic --example native_metal_world_material -j2
    target/debug/examples/native_metal_face_sweep "$RUNNER_TEMP/metal-sweep-input" fresh-native-output
    python3 scripts/check-metal-face-sweep.py docs/audits/face-sweep-source fresh-native-output

Actual GPU artifacts must carry exact source/binary hashes, all failures, device name,4samples, ownership results and literal frames before acceptance.
