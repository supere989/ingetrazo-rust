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

## Native sized CoreText glyph owner

The corrected ARM run at 3ffebfa6 reaches `Paper font outline face failed`: its
valid cached SFNT contains shaping/metric tables, deliberately no outlines.
The new macOS annotation face retains CTFontCreateCopyWithAttributes at the
actual admitted pixel size, shares physical metadata bytes, shapes using that
owner's native advances/metrics, and rasterizes through CoreText/CoreGraphics.
Gray coverage retains the existing native weighted inverted RGB rule; LCD
coverage carries each inverted channel directly into the existing native CAD
operator. No FreeType outline reconstruction or TTC copy is introduced.
Existing paper pixel100 native shaping and grayscale behavior are preserved.

Native allocations/driver tables remain outside the logical encoded-byte
limits; face count and retained-byte admission still happen before creation
and old faces retire only after successful preparation. Gray production does
not allocate an unnecessary RGB coverage copy. LCD output is bounded to three
bytes per admitted pixel. The named Apple consumer now checks real glyphs at
13/17px, nonempty gray/RGB coverage and warm immutable owner reuse before the
GPU roster. Focused ARM Rust check passes; actual Apple execution of this
follow-up is pending, as are strict cross-platform glyph/pixel semantics.

## Resolved font and platform attribution

The actual native-font.json records physical path/TTC index separately from
byte-owner index, resolved CoreText family/PostScript name, SFNT style, native
backend/policy and pixel-sized 1/64 metrics. This is cold diagnostic work only.
The Apple workflow also runs a bounded pinned Qt6.12 RGB32 glyph reference at
13/17px for the source bare-QFont default and the explicit Sans Serif request,
with actual QFontInfo/QRawFont identities, table hashes, metrics and four tiny
images. A supplementary checker reports native/Qt family and metrics; it does
not grant a pixel waiver. The frozen Linux DejaVu Sans Bold full frames remain
strictly compared. Font fallback/hinting and shader errors must be attributed
separately. No font/palette has been fitted to a screenshot.

The Qt venv lives under RUNNER_TEMP and is excluded from artifacts. Its
installation and reference commands have finite deadlines; a failed optional
reference is logged and never suppresses native consumers or strict checks.
Actual glyph comparison/hardware execution still awaits the next Apple run.
Focused byte-owner/diagnostic host test passes; ARM named Rust consumer check
passes; Python AST and workflow YAML validate. Native font/table/CG allocations
remain outside the logical encoded-byte budget, as stated above.

## Terminal sized-font Apple acceptance

Both ARM and Intel runs at b2db5db23ec0d6b8ec418cfa6340132e443aa5cd
complete native shader/build and functional sweep lifecycle controls, including
old-frame retention on font/CAD refusal, retry, real Tool cancellation and final
zero GPU charge. Classic and material sampled controls pass on both. All 24
strict full sweep frames fail on both; exact pixels remain OPEN. Intel receipt
and original artifact join ARM under apple-b2-native-sized. Each receipt pins
160 runner-produced source hashes and explicitly separates six supplementary
Git blob hashes from that runner inventory.

Native CoreText resolves Helvetica-Bold whereas actual macOS Qt6.12 default
and Sans Serif resolve .AppleSystemUIFont Bold, with different metrics. This
is a measured platform/font boundary, not a pixel waiver. Large ordinary-world
interior differences also remain: the source ordinary-prism back overlay is
red while native interiors are blue, and source cream default back surfaces
appear gray natively. These are separate material-policy defects under audit.
No colors, thresholds or frozen source frames were changed.

## Bounded ordinary back-policy diagnostic

The next named consumer emits the actual admitted operation roster per base
frame: prototype/reflection/winding, color phase, first uploaded vertex RGBA,
opacity, cull, depth write/compare and bias. Immutable shared upload-buffer reads
are extent checked and cold only. A separate same-buffer native diagnostic
renders shader front_facing as green/front or red/back while preserving actual
cull/depth/bias/order. It uses the existing finite Diagnostic owner/readback and
quarantines all owners on unknown completion. It then restores the ordinary
render and verifies the complete world+label frame before continuing unchanged
strict source comparisons and lifecycle controls. MSAA boundary/line pixels
are unclassified; colors are not changed to match the source.

Production uses a zero diagnostic uniform field. The emitted fragment shader
adds a uniform diagnostic branch; no normal draw, upload, allocation or pass is
added. Diagnostic-only renders/readbacks and bounded JSON receipts are extra
cold acceptance work, not a production performance improvement. The workflow
now pins the four native font implementation files and both Qt helper scripts
in the runner-produced source inventory as well. Actual Apple result for this
addition is pending; no local crosscheck substitutes for it.

## Actual ordinary/back diagnostic (17931738)

Run38138427433 completed on ARM job114467035078 and Intel job114467034988. Both actual native builds, sweep runtime/lifetime/refusal checks, classic/material consumers and the six-case/five-negative policy diagnostic pass. Both strict full24-frame comparisons fail; exact source pixels remain unchanged. Raw runner artifacts and167 independently verified source hashes per runner are retained in `apple-policy-diagnostic/` with binary hashes and full difference inventories.

The ordinary default-face correction reduces base-frame differences materially (ARM reversed-door2587, back-recess18267; Intel641/15795), while ordinary-prism remains189354/189214 pixels. Actual prism admission records six opaque back-culled draws and one transparent authored-back front-culled draw (opacity0.5, red uploaded RGBA). The actual front-facing mask contains186096 front-facing and zero back-facing prism pixels. Thus the back payload is retained and submitted; source/native face-side or coverage policy still requires diagnosis. This evidence does not justify flipping winding or fitting color. Native font identity/hinting and MSAA resolve rounding remain separately attributed strict boundaries. No additional CI retry is claimed or requested by this receipt.

## Actual consolidated-guard execution (1fa6c83a)

Run38139827943 completed ARM114471208486 and Intel114471208781. Both builds, actual sweep lifecycle/refusal/retry, six material-policy diagnostics/five negatives, classic and material consumers pass. Strict24 full frames still fail. Raw artifacts,168 source hashes independently verified per runner, binary hashes and unchanged strict inventories are retained in `apple-consolidated-guard/`.

The guard changes the actual prism mask to0front/184744back on ARM and0front/184520back on Intel, confirming the source-selected side now executes. Dominant source prism colors227/152/140,203/148/138,201/146/138 compare with identical ARM/Intel native227/151/139,203/147/137,201/145/137. The large remaining prism count is predominantly green/blue -1 interior rounding, alongside existing edge/font/coverage differences. Original basic.frag emits raw `u_opacity` (line171; sampled alpha line157); Metal currently pre-rounds .5 to128/255 in source_blend_alpha. This quantization predicts the observed lower green/blue channels and is a concrete next source-law investigation, not permission to fit palettes or relax comparison. No additional retry was run.
