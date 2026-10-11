# Source-owned edge lookup

Genuine native Mesh.find_edge prerequisite, c74 graph dependency plus 23fc56a3.
Actual frozen source and native agree on seventeen ordered observations: reverse identity, public-list-detached lookup, empty smaller incidence, equal/asymmetric degree choice, length callback changing the other slot, dynamic other callback/membership assignment and raised error propagation, mapping/list incidence.

The method clones short slot references and releases locks before Python length/iteration/other calls; it iterates only the source-selected smaller incidence, not the whole public Edge list or a cloned set. A fresh incidence read follows length callbacks, matching the actual source expression. Native fixture tears down controller and verifies both logical ledger and graph payload counts zero.

Focused named build only:

```
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build --manifest-path tools/ai-interpreter-proof/Cargo.toml --bin vm_graph_find_edge -j2
/tmp/ingetrazo-ai-registry-target/debug/vm_graph_find_edge > NATIVE
python scripts/check-ai-graph-find-edge.py docs/audits/ai-graph-find-edge/original.json NATIVE
```

Five mutated outcomes are passed through the same strict validator and rejected. No tolerance or source operator was changed. Initial fourteen-case proof was expanded to mapping/list incidence before freeze; general object length replaced sequence-only length.

This adds an actual full helper operation but does not enable production scripting. Transaction snapshots still currently require builtin set/list slots after execution, although this read-only helper supports arbitrary live iterables transiently. Public graph deletion/replacement/endpoint projection, arbitrary duck-typed vertex arguments, registry detachment, full history/callback side effects and VM/native resource bounds remain required. Snapshot capture equality defects from the graph stage stay open. No whole-graph or total-memory/deadline claim.
