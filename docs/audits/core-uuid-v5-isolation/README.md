# Standalone core UUID-v5 feature repair

The saved-view source identity helper moved `Uuid::new_v5` into core without declaring core's `uuid/v5` dependency feature. Whole-workspace builds obtained that feature from formats and masked the failure. A fresh worktree at dc1ee0cf reproduced E0599 in standalone core lib and lib-test compilation, exit101. The one-line df9ad4ad change explicitly enables v5 in core; Cargo.lock and runtime algorithms are unchanged.

Focused checks only, using the existing exclusive dependency target with local packages selectively invalidated at worktree handoff:
```
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo check --locked -j2 -p ingetrazo-core --all-targets
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo check --locked -j2 -p ingetrazo-core -p ingetrazo-metal --all-targets --target aarch64-apple-darwin
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo check --locked -j2 -p ingetrazo-core -p ingetrazo-metal --all-targets --target x86_64-apple-darwin
```

All three post-fix checks PASS: host10.63s, AppleARM1m34s, AppleIntel1m27s. Baseline/post-fix logs are compressed and byte-verified, with hashes in provenance.json. Original resource symlink points to /work/ingetrazo. This is cross-compilation acceptance, not actual Mac GPU execution or framework-link proof. No broad suite, tracker edit or push.
