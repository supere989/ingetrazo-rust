# Persistent direct Vulkan transfer staging review

Base: 51940fd8f8ca48c0e64148b035a4c427a8ec6c2f. Implementation and acceptance remain OPEN.

Current source inspection finds buffer upload allocates and destroys staging per call; upload_many does so per batch and maps/unmaps every submission. Images already retain a lazily grown, persistently mapped allocation bounded at 8 MiB. All three upload paths share one completed command/fence owner. Readback is a separate diagnostic path.

Required change: reuse the existing bounded image staging owner for buffer and batch uploads. Check resource/context lifetime before writing any retained staging. Grow only after completed transfers; preserve quarantine retention after uncertain submission. Batch scatter copies directly into the retained mapping and flushes noncoherent memory without a packed CPU buffer. No new render pass, process, queue wrapper or scene copy.

Acceptance must exercise increasing/smaller/interleaved image and buffer requests, >8 MiB chunking, exact GPU readback, one retained command/fence, stable allocation/map counts after capacity is established, admission rollback and uncertain-fence retention. Retained staging remains budget-charged until owner drop; update diagnostics that currently demand zero bytes immediately after destination retirement, retaining a zero-ledger assertion after owner drop. Keep per-case upload timings separate from readback and do not infer whole-render throughput from upload timings. Actual discrete GPU proof is required alongside software controls.

Affected diagnostics identified: native_vulkan_upload_churn, native_vulkan_resources_probe, native_vulkan_headless_probe and native_vulkan_texture_upload_probe. Timeout quarantine and budget-denial controls must be rerun; the existing texture map counters must keep their declared meaning or be replaced with explicitly general staging counters. All parity rows/release gates remain OPEN.

Build policy: debug=0, incremental=0, -j2; named diagnostics only. Commit evidence before purging terminal temporary outputs and targets.
