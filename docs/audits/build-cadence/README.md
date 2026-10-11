# Avoid redundant example linking and dependency rebuilds

The seven-package workspace at `d76eb668` declares 219 example targets and none has `test = true`. Cargo's default test command builds examples to check compilation, so repeatedly changing shared GUI code also relinks every diagnostic example. The canonical workspace check now runs all test targets, all library doctests, and checks every example without linking it. Named runtime acceptance scenarios continue to build and execute their selected examples. No test target or runtime acceptance step was removed.

The three existing CI workflows now reuse compiled dependency artifacts through a commit-pinned Rust cache action. Compiler/target/manifests/configuration form cache keys; cross-target jobs also include their target explicitly. Workspace crates and installed Cargo binaries are excluded from saved caches. CI disables debug symbols and incremental data. Existing workflow/ref concurrency cancels superseded jobs; tracker-only pushes skip broad acceptance. The next integration run will populate caches, so no measured speedup is claimed yet.

References: [Cargo test target selection](https://doc.rust-lang.org/cargo/commands/cargo-test.html#target-selection), [Rust cache behavior](https://github.com/Swatinem/rust-cache/tree/49a0bdc70d2e1b713ca9e2869b211fcce03d3c1c).
