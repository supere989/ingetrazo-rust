# Cached material viewport stage

Source dependency order (after classic stage a6f78831):

- 2e494b91: material/sampled asset actual Scene consumer.
- c7356c84: frozen original recipes and actual update_ui frame protocol.
- c7bd526d: isolated SDK workflow only.
- 30bcdb36: original shaded opaque-front mean; glass/back stay sampled.
- 919a9ba1: immutable material layout identity and accepted-frame gate.
- 8bbbe892: private staging upload ownership and encoded source alpha factors.
- cf6e6ffa: per-run uniform opacity preserves source nearest-even ties.
- 48f5e960: actual tracked native heap backing admission and shared two-asset proof.
- 4effad9d: bounded retained immutable assets across untextured scenes.

Failure-only evidence commits: 27a898f2, 8f32467f, 72da1b26, 5a80beb1, 799c3466, f7b34744. They retain the cache panic, placeholder frames, numeric standalone query mismatch, zero-query Intel refusals, and weak-cache reupload failure.

The final source is 4effad9d3eabbab55e79adedda1a5e48aa36bb5f, run 38111448432. ARM passes actual SDK/MSL/app rendering, 25 original recipes × three frames, 1,950 exact RGBA sample pixels, two immutable uploads, one 4 MiB backing heap for two assets, warm counters, and final ledger 0/0. All 47 source hashes and binary/archive hashes are retained. Replay:

```sh
python3 docs/audits/metal-world-scene/material-run38111448432/replay-artifact.py docs/audits/metal-world-scene/material-run38111448432/arm.zip
```

The checker also rejects blank/one-pixel/stale-back/missing-warm/heap-owner/uncleared-ledger mutations of the actual accepted artifact. Exactness is sampled interiors, not full-image, mip filtering, edges, UI gestures, presentation, or physical Apple hardware certification. Intel SDK build passes but the first target query remains size0/alignment0 on Apple Paravirtual, so strict preallocation refusal yields zero rendered frames. This platform gate stays open. Full materials and native viewport parity remain open beyond this supported stage.

GPU ownership: full native heap capacity is admitted before creation, texture placement uses native heap queries, immutable assets own the backing heap, and existing Prepared readers retain assets. Unknown upload completion quarantines command, staging, image and heap together. Cache is limited to 4,096 identity entries plus shared GPU budget; explicit retirement releases assets while readers remain safe. Lookup-byte stats exclude native ObjC records/allocator overhead and are not total memory claims. No per-frame decode or full Scene clone is introduced.
