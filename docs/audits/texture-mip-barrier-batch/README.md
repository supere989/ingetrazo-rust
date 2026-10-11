# Completed mip-layout barrier batching

Production keeps each dependent blit barrier, then batches shader-ready layout ranges after the last blit. Scatter recording uses fixed 256-record storage; single-image recording uses two records through the same const-generic builder. No image, sampler, blit, shader, mip bytes or fence-publication policy changed.

Actual RTX 5090 baseline/candidate captures retain all 38 levels from four real assets: every byte is unchanged, while mip barrier API calls decrease 72→38. Two production World frames match retained original MSAA frames exactly; 20 warm updates add no uploads. Actual budget refusal preserves the frame, retry succeeds, and final ledgers are zero. The 17 MiB control retains exact GPU-produced base bytes, three transfers and an 8 MiB staging peak; mip calls decrease 23→12. These are command-count improvements, not measured throughput claims.

Odd-dimension source parity remains FAIL: summed differing pixels/max channel error are 1020/9, 28094/10, 5596/72, 4876/58. All level-zero images remain exact. The bounded NVIDIA reciprocal observer refutes the tested coordinate hypotheses; it does not establish a general proprietary mip law. Initial observer setup failure is retained separately.

`captures.zip` stores deduplicated payloads. `manifest.json` authenticates every logical baseline/candidate raw record, protocol, build log, executable hash and prior reference archive. Following parent independent replay and release, authenticated terminal raw directories and seven frozen executables were removed (120,199,564 bytes); cleanup.json records exact paths, sizes and executable hashes. Canonical archives, source and reusable dependency target remain. No source recapture or new timing series occurred.

The final const-generic control accidentally reused the full four-asset input: its recorded transport and output are authoritative, and all 38 levels remain identical. The archived one-case derived JSON was not used; it is not acceptance evidence. No additional World recapture occurred.

Read-only replay (NumPy and Pillow):

```
/tmp/ingetrazo-parity-python/bin/python scripts/check-texture-mip-barrier-batch.py
```

Frozen runtime order from dc1ee0cf: 4e4f6aa3 (counter/protocol), 270a9f50 (barrier batching), 461d6352 (single-image storage). The checker includes altered-byte and truncated-input negatives and references prior source captures rather than duplicating them.
