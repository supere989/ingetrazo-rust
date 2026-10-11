# Closed-only Intel ownership diagnostic

Run38164613793, source bbc3ffa62eab33b1b090c987e4e4c3daa19f6e3a. Build, four-cell runtime, inventory and artifact upload passed. Archive11711941125 SHA256 e6d07dbbf7d27c1d845037152ad01912034450c2fb5107e557723bf2d48d2d9d; all53 listed source files authenticated against Git objects. Binary SHA256 e15c81dfe8c97db92fb462a7c910fc153961c786807318f264cdd80e9811680b.

All20 whole RGBA files are1,920,000 bytes. All4 seed frames and both native cells' first+3warm frames equal the previous9302 reference byte-for-byte. No zero-alpha pixels occur. Both compute cells'4 frames are identical to each other but differ from native at288 pixels. The original all-zero9302 candidate remains an unresolved, non-reproduced failure. Neither Adapter reuse nor missing synchronization is established as cause; compute remains diagnostic-only. No retry or production fix is justified by this result.

Snapshots acknowledge Completed, actual Shared readback, Private color/MS/depth, Clear with identical clear bits, native MultisampleResolve versus compute Store and identical device/queue across cells. Two fields (`resolve_destination`, `readback.identity`) erroneously record wrapper addresses; do not interpret these as native resource identity. A separate source-only dereference repair is for future observations and does not change this artifact.

`independent-review.json` independently counts every complete pixel and retains frame digests. The authenticated ZIP retains all first/warm bytes, per-frame snapshots, exact source/binary digests and build/runtime logs. Scope: one Intel runner's closed case; no whole section, physical presentation or release claim.
