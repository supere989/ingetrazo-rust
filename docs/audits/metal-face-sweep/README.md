# Direct native Metal borrowed face sweep consumer

The application passes its actual Tool::face_sweep_preview to the native world route with the Qt-float camera eye. Overlay caps/walls use fixed front paint and front texture on both sides, after ordinary surfaces/edges/ghosts; prism replacements use ordinary materials and layers. Ordered endpoint lines use the original GL_LINES width1 contract, compact12-byte positions, and the existing world render pass. No authoritative Scene mutation, managed image wrapper, extra render process or readback is used in production.

The World retains at most the original and suppressed base meshes for the active acquisition. Immutable offset geometry changes upload the preview prototype and its endpoint buffer; unchanged source prototypes reuse existing native buffers. Camera-side suppression reuses cap/wall GPU keys. Cancel finds the original cached geometry. All candidates are private until complete admission. Prepared owners retain matching buffers/images/pipelines/targets; shared budget charges cover extra base variants. base_variant_gpu_bytes reports extra buffer residency excluding current placements and shared images. CPU descriptor traversal and material ordering are still O(prototypes/operations); this is not a globally O(changed) renderer. CPU preview rebuilding on a suppression-side change remains O(affected geometry).

Focused host cache test passes1/1 (offset variant retention, camera-side GPU key reuse, invalid acquisition and unchanged original packet). ARM Rust library/actual app diagnostic type checking passes. Initial GUI crosscheck lacked a Mac zlib toolchain; final Rust check used host C with default target C flags disabled solely to unblock cargo check. This does not establish Mac linking or shader compilation. No actual Metal preview execution or strict pixels are claimed yet.

The new native_metal_face_sweep executable consumes the authenticated six-source-case archive: actual app click/hover acquisition, base/preview/offset/Cancel, immutable model/history, repeated-frame0geometry/line uploads, offset-only1prototype/no base-line/no image uploads, admission pressure preserving old pixels/charges and retry, cached Cancel, final ledger0. These are assertions awaiting real Apple execution. The checker retains strict24wholeRGBA comparisons and six malformed/pixel negatives. The subsequent label consumer now composes the original two text layers through the shared native font cache and exact operator-tagged CAD callback. The diagnostic reads back world plus label in the same production painter ordering; no label pixels are masked. This complete pixel gate remains unproven pending Apple execution, alongside previously quantified vendor MSAA boundaries.

Existing section/shadow/image/annotation/profile/grid and richer edge modes retain explicit admission boundaries. Production UI/OS gesture, all preview styles and full arbitrary geometry parity remain required beyond this bounded corpus. The source oracle is reused from docs/audits/face-sweep-label-source; no source recapture.

Apple workflow restores both runners and builds four named consumers in one command; the existing classic/material/ghost controls run alongside this shader change. Execute:

    python3 scripts/check-metal-face-sweep.py docs/audits/face-sweep-label-source "$RUNNER_TEMP/metal-sweep-input" --unpack
    cargo build --locked -p ingetrazo-gui --example native_metal_face_sweep --example native_metal_hidden_ghost --example native_metal_world_classic --example native_metal_world_material -j2
    target/debug/examples/native_metal_face_sweep "$RUNNER_TEMP/metal-sweep-input" fresh-native-output
    python3 scripts/check-metal-face-sweep.py docs/audits/face-sweep-label-source fresh-native-output

Actual GPU artifacts must carry exact source/binary hashes, all failures, device name,4samples, ownership results and literal frames before acceptance.

## Label consumer prerequisite

The corrected source archive bdee46b3 contains the complete DejaVu Sans Bold 13px/96dpi/DPR1 reference and retained fallback-font failures. Every archive record is authenticated before extracting only complete-config inputs. Shared raw tool hook and PushPull midpoint determine the label. The neutral producer retains straight source RGBA and explicit QRgba64/LCD operators; generic CAD callbacks retain their prior scalar span contract. Both Apple framebuffer-fetch and Intel ordered span compute consume the glyph tag. LCD follows the original per-channel GL source blend expression; actual native rounding remains an execution gate, not a tolerance or palette compensation. The compute palette retains operator ownership in its cache identity.

The application selects the existing exact compute painter for world labels on non-fetch devices. Warm plan/geometry/palette upload counters are checked; offset pressure keeps the previous label together with the previous world. A host operator-inventory admission test passes 1/1 and ARM Rust consumer checking passes. Mac shader compilation, actual final whole-frame label pixels and native GPU failure/retry are awaiting the joint two-runner job. The followup now prepares the font plan and native CAD owner privately before World preparation and publishes the matching label only after the world succeeds. Actual execution is still pending: the diagnostic asserts a real one-byte font-cache quota refusal and shared GPU-budget CAD refusal preserve exact previous complete pixels and GPU charges, retain the old native callback through Weak observers, and release it after completed replacement. World refusal drops its private label candidate. No unsafe thread traits were added: egui carries a scalar token while the owning native painter installs one prepared owner slot. Full annotation depth-gate operations, large arbitrary labels and total VM/native allocator bounds remain outside this bounded distance-label slice.

Unchanged label frames reuse the prepared native callback with no preparation/upload. Changed labels allocate their own ordered geometry/palette and reuse the accepted native pipeline; no per-hover shader compilation. The app still visits the label key and clones its typed string when nonempty, and the painter installs one Arc owner slot per frame. CPU font-cache preparation can retain new glyph masks after a later world refusal; this does not publish label geometry or retain candidate GPU charges. Pipeline/compiler internal allocations are outside the shared buffer charge accounting, as in the existing CAD consumer. Host ARM Rust checking passes; the strengthened GPU refusal/lifetime assertions are compiled but awaiting the joint Apple run.

## Shared shader ABI followup

Both direct MetalCad2d and inline callback draws now upload the same DrawPush type: 64 bytes, rgba at byte32, glyph at byte48. Plain CAD always sets glyph=[0;4]. The host layout/zero-operator control passes. The separate cad2d_spans ordered direct compute shader retains its original48-byte ComputePush and uint4 palette; cad2d_compose callback compute retains its48-byte push with the explicit48-byte glyph palette record. All include/upload sites were audited; the earlier direct48-byte draw upload was a real regression caught before Apple execution.

Immutable glyph admission retains both batches and operators by Arc identity. Two thousand warmed snapshots cause one operator scan; new unknown/missing inventories reject without invalidating the previously admitted pair. The two focused host controls and ARM app consumer check pass. Accepted native label records are immutable; each changed private candidate gets a new producer snapshot and fresh buffers. Only pipeline Arcs are shared with the accepted owner, so candidate discard cannot mutate the previous callback. These controls do not establish actual Metal shader or hardware execution.

## Actual Apple label admission follow-up

Run 38135216541 at d74e0f4c built all four named consumers on ARM.
The sweep stopped at reversed-door/preview with `Invalid paper font`; classic
and material runtime/checkers passed. CoreText extracts a standalone SFNT,
while Match.index identifies its face in the physical TTC. Annotation sized
faces now decode with the cached byte owner's face_index (zero on that path),
without changing physical identity. The focused Linux regression deliberately
sets the physical index to 37: old decoding refuses, owner-index decoding
succeeds. This is a decoding regression control, not Apple GPU acceptance.

The source base/cancel frame protocol retains the acquired PushPull tool at
zero distance, including its label. The native diagnostic now uses that same
actual tool hover rather than switching to Select. Actual tool cancellation
is distinct from these source frame names. Strict pixels remain OPEN pending
the corrected consumer's Apple execution; no reference or tolerance changed.

Actual Tool cancellation is now a distinct assertion after each source roster:
set_tool(Select), cached base geometry upload counters unchanged, preview and
label slots empty, and completed old label Weak owner released. This assertion
has no source-frame pixel comparison (source cancel means zero-offset acquisition).
The strict checker requires its result and rejects false cancellation metadata.
It awaits actual Apple execution together with the font correction.

Corrected named ARM Rust consumer check passed (9.79 seconds, host C/C++
shortcut only; not native link/shader execution). The first local invocation
failed because host C++ received Apple -arch flags; both logs are retained.
Both d74 Apple jobs are terminal failures with the same font admission error;
160 source hashes per artifact independently match d74. Classic/material
checkers pass on both; ghost runtime passes while strict pixels remain OPEN.
Receipts and original downloaded artifacts are under apple-d74-failure.
