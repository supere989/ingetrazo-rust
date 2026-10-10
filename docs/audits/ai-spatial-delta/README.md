# Reversible spatial delta proof

Source `1dc2fbfd`, parent persistent registry `71a95502`. Standalone only. Design/boundaries in docs/design/AI_APPEND_SPATIAL_DELTA.md.

22 focused owner tests PASS, including collision restore, stale tokens, quota refusal/Undo/Redo/history retention. Four actual null allocator injections hit ordered-position, delta-key, temporary-map, and live-map reservation; all returned refusal before mutation, preserved exact mesh/index, and retry/restore passed. This is controlled allocator failure, not OS OOM or VM allocation admission.

Twelve native history states and seven registry-backed weld cases exactly equal retained actual pinned original outputs. Same validator rejects six malformed/mutated controls.

Actual allocation measurement, with live Mesh spare capacity and one cold registry growth primed: both 1,000-vertex and 50,000-vertex bases execute 64 appends, 64 Undo and 64 Redo with 1,869 allocation calls / 258,696 allocated bytes / largest request 11,776 bytes. Initial capacity growth is separately recorded: 52,288 vs 2,404,288 bytes. This proves bounded warm work in this fixture, not zero allocation, universal latency, no capacity-growth copying, or full scripting performance.

Focused commands in tools/ai-interpreter-proof, inside ingetrazo-publish:
```
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target cargo test -j2 --no-default-features --bin owner
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target cargo run -j2 --no-default-features --bin index_failure
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target cargo run -j2 --no-default-features --bin index_scaling
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target cargo run -j2 --no-default-features --bin owner_history
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target cargo run -j2 --no-default-features --bin owner_weld
```
Then from worktree root: `python3 scripts/check-ai-spatial-delta.py`.

No VM/broad suite. Production disabled. Initial scaling assertion failure is retained separately: before switching registry position growth from exact to amortized and separately measuring initial capacity growth, the largest-allocation bound failed. The correction avoids repeated exact-size base copies and reports initial growth truthfully.
