# Sheet image export workstream

Seven actual original PNG/JPEG exports cover physical sizes, orientation, resolution, shape layering/hidden items and text. Six original handler controls establish cancellation, extension selection, remembered DPI and failed-write ordering. Dialog answers and frame-refresh counters are supplied controls, not desktop dialog or frame-rendering proof. Source images and metadata are frozen once.

The native codec consumes opaque RGBA8 directly: PNG streams one RGB row; JPEG borrows the RGBA pixels without a second full-page copy. Dimensions use source nearest-even rounding and physical density metadata. Atomic publication preserves destinations on failure. Codec acceptance is pending; native GPU page preparation, tiled export and GUI integration remain in progress. No P29 promotion or native image-rendering claim.

## Codec checkpoint and implementation handoff

At `f32661a7`, the PNG/JPEG writer compiles locked/offline and preserves physical dimensions, DPI metadata, opaque input admission and an existing destination on refused input. Five PNG fixtures retain exact decoded source pixels. Blank JPEG pixels match; colored JPEG is not byte-exact: after correcting the observer to encode the frozen lossless `shapes.png` rather than recompressing the original JPEG, 19,634 decoded pixels differ (maximum channel delta 12). JPEG SOF component sampling and both quality-92 quantization tables are identical; `jpeg-headers.json` records the literal markers. This isolates the remaining decoded mismatch to the encoder implementation, rather than a quality or sampling-setting difference. Both generations and reports are retained in `codec-records.zip`; this is no renderer acceptance or full JPEG parity claim. The observer-only rebuild took 0.266s and selected JPEG execution 0.163s. No original recapture.

Next deliverable is a consuming image-export workflow through the existing native GPU host, with direct Vulkan on Linux/Windows and Metal on macOS. Reuse the existing CAD callbacks, text/font producers, resource budgets, lifetime handling and page item order. Keep device/renderer owners cached and tile images that exceed device extent; avoid full-page RGB copies, external PDF conversion and per-export subprocess startup. Existing `composer_pdf` view/text preparation can share physical geometry, but currently rejects raster/perspective views, titles and model annotations. Do not silently omit unsupported items or claim the canvas fallback is exact. Existing canvas source paper includes editor margins/shadow/selection, which must not appear in exported images.

Original handler contract is frozen in the source archive: refresh before chooser; recognized suffix wins; otherwise append the selected PNG/JPEG suffix; default DPI 200 persisted under `composer/image_dpi`, range 50..1200; cancel at either chooser writes nothing; store DPI before encoding even on writer failure. Original painting uses white RGB32 paper, nearest-even pixel size/DPM and `_paint_sheet` ordered visible items plus border on top. `reference-sheet-image.py` counts refresh invocation only. Seven images and six handler controls are scoped source observations, not the entire item/rendering inventory.

## Native consuming workflow (989a2809)

The direct native Vulkan consuming workflow passed all eleven decoded source images exactly, including JPEG quality92 and PNG/JPEG physical density headers, first on llvmpipe and then on NVIDIA GeForce RTX 5090. The hardware run took 4.353 seconds. The final cached affected build took 6.17 seconds; earlier coherent build and narrow repairs/failures are retained. libjpeg-turbo3.1.0 was built without SIMD because NASM was absent; this is not an encoder performance claim.

Reuse immutable `source-records.zip` for the original seven images/six chooser controls. `view-source-records.zip` adds four actual physical View sessions (current, saved, cropped, hidden dashed/thin border); its manifest uses `archive_sha256` and `entries` with path/bytes/sha256. `native-records.zip` retains first failures, final software/hardware output, transport identities, compiler logs and final source/binary provenance. Its manifest uses the same schema. First WIP binary bytes were not separately frozen; only final binaries are authenticated.

Extract original source to SOURCE, View addendum to VIEWS, then run:

```sh
sheet_image_workflows SOURCE OUT --views VIEWS --worker ingetrazo-render-worker
python scripts/check-sheet-image-native.py SOURCE OUT OUT/check.json VIEWS
```

Default device admission requires NVIDIA; `--software` explicitly enables the llvmpipe diagnostic. The actual App workflow covers path/resolution cancellation, remembered DPI, same-frame modal ownership, UUID-stale refusal, unsupported painter destination preservation, late JPEG row failure and cleanup. Row-band GPU readback/encoding avoids full-page RGBA allocation; the 2339-pixel image crosses the cached2048-pixel tile target. Larger-than-hardware-limit pages remain code-supported but unobserved.

This is the eleven-image Linux Vulkan slice, not full P29. Native Metal/Windows submission and actual desktop chooser acceptance remain unproved. Visible raster/perspective Views, title/scale text/model annotations, titleblock/north arrow/graphic scale, images/legends/dimensions/labels/profiles/levels/callouts and unsupported vector fill/join/clip forms refuse before destination modification. Mac text needs its own Qt/font oracle; Linux Sans Serif is not a Mac Helvetica oracle.
