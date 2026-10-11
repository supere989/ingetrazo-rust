# Direct Vulkan cold image-submission batching

Runtime source: `7a4703455e6479b3ef551a9b03aa1025ecf6e35c`, based on `a9cd0eea`. Frozen original: `6be29fe437a7cd992d4e079f71720821b25216b9`. `captures.zip` holds all 52 raw reference/consumer/diagnostic/protocol records; `manifest.json` authenticates every member. No shader or sampling law changed.

The original `_get_texture` uses mirrored QImage, Repeat, LinearMipMapLinear/Linear. Its low-coverage cutout policy uses Linear minification, so the native exact BC7 single-level path is preserved. Other native images already have GPU-generated mip chains; this change batches their cold uploads through the existing bounded persistent mapped staging allocation. It does **not** solve the retained odd-size NVIDIA GL mip-generation differences in `docs/audits/texture-mip-source-diagnosis`.

World/reference preparation retains unpublished image owners and their immutable decoded payloads, writes those payloads directly into staging (no packed CPU byte copy), and submits multiple image copies/mip barriers together. Transfer completion precedes descriptor publication. Queues flush at 8 MiB pending payloads; retained CPU payload peak is below 8 MiB plus one separately admitted incoming image, excluding that decoder's existing work budget. Completed candidate suffixes are not rescanned. Cache hits allocate no batch pointer arrays. New/replacement images, staging and old resident images all use the existing actual VkMemoryRequirements ledger; unknown completion retains owners through existing quarantine. No new per-frame upload, Scene copy or external image process is introduced.

Actual NVIDIA RTX5090 production World MSAA4 near/far captures each match the full original 800×600 image exactly (30 source sample points also exact). Four power-of-two source mip chains plus a no-mip exact BC7 cutout require five image uploads, 26 generated levels, 315,456 uploaded bytes. Cold image fence submissions drop from 5 to 1; actual staging allocation increases from 262,144 to 315,536 bytes. Both consumers retain the same frame after real image-budget refusal, retry exactly, and release the ledger to zero. Twenty warm updates add zero transfers/uploads/maps. Prior consumer observation `0aa1752a8060a54d75f1943a14bd5a1ea3d83927` adds only the same probe/accessor/counters; its exact patch is archived.

**No throughput speed claim:** the comparable full `World::set_scene` cold wall-time measurements are old 7.36107 ms/new 13.463348 ms, a single sample each; the new sample is slower and the pair is not a benchmark. Internal TextureStats.upload_ms is not a comparable timing boundary after deferred preparation. No GPU clock or utilization measurement was made. Fence count alone does not establish higher throughput.

A separate actual cached 2048×2176 constant RGBA image exercises 17,825,792 base bytes, three bounded transfers, 11 GPU-generated mips, 24,424,448 actual image-allocation bytes and 8,388,608 staging bytes. Every GPU-read base byte matches, the mapped owner remains maps1/unmaps0 while warm, a real budget denial preserves the published image, and completed-fence eviction frees the image before final ledger0. Readback is diagnostic only. This proves the changed multi-chunk transfer path, not full large-image streaming or odd-size mip parity.

Historical single-sample captures are retained with strict 1,902/573 differing pixels; they are not source-parity evidence. The corrected reference explicitly records the actual Scene FBO at four samples, and its images are byte-identical to the earlier original captures. All owned seats and disposable GPU processes were stopped. Executable hashes/source macros and commands are in the archive; binaries themselves are not committed.

Read-only replay:

```sh
podman exec ingetrazo-publish /tmp/ingetrazo-parity-python/bin/python /tmp/ingetrazo-source-mip-consumer/scripts/check-texture-upload-batching.py
```

The checker verifies every archive hash, mandatory case/asset inventory, full extents, source filter policy, full-frame equality, old/new counters, warm/refusal/retirement/ledger controls and literal altered-pixel/missing-case negatives. It does not run a GPU or extract the archive.

Remaining scope includes exact odd-size source mip generation, asynchronous upload scheduling/streaming beyond current decoded admission, further formats/compressed authored mip chains, full GPU utilization/throughput measurements, platform portability and full renderer parity. Actual shared-application UI lifecycle was not rerun in this batch; the production World consumer is the exercised path.
