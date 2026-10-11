# Application consumer integration checkpoint

Runtime source `282bf8ad42f586eee3da03f4f7f7c800672e6370` passes the revised workspace check: **784 passed, zero failed, two ignored across 35 test/doc targets**, with all enabled examples compile-checked. The complete command took **21.13 seconds on the reused target**. This is an observed warm run, not a cold benchmark or a measured speedup ratio. Selected acceptance examples still compile and execute separately.

The optional `engineering-python` feature passes three focused actual-application tests and its merged console fixture matches four original states with five rejected mutations. It remains off by default. Full Scene scripting, external history synchronization and total resource bounds remain open. The console run took 56.794 seconds including its focused test/build/check sequence.

A borrowed build target initially reused older local-path core and renderer crates from another worktree. Their dependency files exposed the stale paths. Selective removal of nine local package artifacts preserved external dependencies; the retry passed. Owners must invalidate changed local package artifacts when handing a target to another checkout. The complete failure, cleanup and successful logs are retained byte-for-byte in `logs.zip`, independently checked against `logs-manifest.json`.

New reviewed integration also includes the opt-in native Metal application viewport and loose-scene consumer, source-scoped ARM runtime evidence, and explicit Intel native allocation-query failures. Those independent SDK artifacts retain their recorded source hashes. Existing whole-model rendering failures and exact annotation proofs have not been rerun or relabeled. The later classic-group Metal and annotation stencil branches are outside this checkpoint.

See `acceptance.json`, `workspace-run.json`, `console-run.json`, and `console-check.json`. All five release gates remain open. Test counts do not measure percentage parity.
