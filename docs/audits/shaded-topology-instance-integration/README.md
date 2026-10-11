# Shaded textures, topology history and instance rendering batch

Production Vulkan follows the actual pinned shader/pass behavior: opaque fronts use a flat texture mean, without multiplying shade; all back overrides and translucent fronts retain sampling. The six-case actual NVIDIA fixture matches 30 source RGBA samples, with one geometry upload per asset, at most one shared sampled image, no sampled image for opaque-only shading, and no warm hashes, decodes or uploads. A mixed asset owns both its mean and its one sampled image.

The full Chapel iso remains a strict failure: 1,065 on/restored pixels and 1,112 off pixels differ, maximum channel error 37. All 3,830 annotation-union pixels match, masks match and native restoration is exact. Earlier phase-only and five-case failures are retained. Original widget frames are referenced from `../annotation-widget-neutral-policy/capilla_quiroz_v17-iso/original`, without copying its operations again. This is one camera, not full model/annotation/platform acceptance.

Independent fresh original and frozen-binary replay matches 18 Edge and 31 Face history states; 27 mutations reject. These standalone scripting routes remain disabled in production. Complete Vertex/reference closure, arbitrary payload/callback handling and total VM memory admission remain open. Exact binary and source artifact hashes are retained without duplicating large JSON state records.

Root independently verifies all source hashes and raw pixels in the ARM nine-frame cache and fourteen-frame instancing archives. Camera-only instance uploads are zero; model pose updates reuse one buffer. Intel SDK execution refuses before allocation because native texture size/alignment queries return 0/0; no Intel frame is claimed. The later global prototype grouping has a separate three-frame acceptance run pending.

`acceptance.json` records the source and combined workspace result; later small admission-search changes use focused tests. `PARITY_PROGRESS.md` remains the canonical feature/release tracker. No gate is promoted by this bounded batch.
