# Persistent direct Vulkan transfer staging review

Base: 51940fd8f8ca48c0e64148b035a4c427a8ec6c2f. Implementation and acceptance remain OPEN.

Current source inspection finds buffer upload allocates and destroys staging per call; upload_many does so per batch and maps/unmaps every submission. Images already retain a lazily grown, persistently mapped allocation bounded at 8 MiB. All three upload paths share one completed command/fence owner. Readback is a separate diagnostic path.

Required change: reuse the existing bounded image staging owner for buffer and batch uploads. Check resource/context lifetime before writing any retained staging. Grow only after completed transfers; preserve quarantine retention after uncertain submission. Batch scatter copies directly into the retained mapping and flushes noncoherent memory without a packed CPU buffer. No new render pass, process, queue wrapper or scene copy.

Acceptance must exercise increasing/smaller/interleaved image and buffer requests, >8 MiB chunking, exact GPU readback, one retained command/fence, stable allocation/map counts after capacity is established, admission rollback and uncertain-fence retention. Retained staging remains budget-charged until owner drop; update diagnostics that currently demand zero bytes immediately after destination retirement, retaining a zero-ledger assertion after owner drop. Keep per-case upload timings separate from readback and do not infer whole-render throughput from upload timings. Actual discrete GPU proof is required alongside software controls.

Affected diagnostics identified: native_vulkan_upload_churn, native_vulkan_resources_probe, native_vulkan_headless_probe and native_vulkan_texture_upload_probe. Timeout quarantine and budget-denial controls must be rerun; the existing texture map counters must keep their declared meaning or be replaced with explicitly general staging counters. All parity rows/release gates remain OPEN.

Build policy: debug=0, incremental=0, -j2; named diagnostics only. Commit evidence before purging terminal temporary outputs and targets.

Draft implementation now shares the retained staging allocation across all upload paths and validates lifetime/context before buffer admission. Exact post-edit GUI library compile passes (compile-final.log); GPU readback/churn/quarantine acceptance is pending. Legacy image_staging_maps/unmaps counters currently count this shared upload staging, and must be renamed or explicitly documented before integration. No production/main adoption yet.

Focused acceptance: software and strict RTX5090 interleaved17MiB image/buffer plus smaller batch exact readbacks pass, with no new staging allocation/map or transfer owner. Software/RTX40 churn rows pass one retained staging allocation per owner. Software real unsignaled-fence TIMEOUT refuses reuse and retains charged owners. Renamed maps/unmaps counters to upload_staging_* for shared semantics. Direct default-container hardware request rejects absent NVIDIA rather than silently using CPU; retained failure precedes explicit GPU transport success. Whole-world regressions, actually queued timeout control, compiled old-path negative, and canonical integration remain OPEN. Hardware transport purged after logs and exact binary hashes retained.
