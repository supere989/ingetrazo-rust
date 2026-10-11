# Prepared changed-row engineering console publication

This optional RustPython consumer prepares a native Mesh row ticket and applies only changed/appended/truncated Vertex, Edge and Face rows. Existing unchanged payloads remain in place; native full-mesh export count is zero. Pose/count changes prepare and validate the full spatial index before application mutation. Material-only commands skip it. One existing application Scene Undo snapshot remains per changed command.

The 14-state pinned original corpus observes the actual document immediately after commands and history steps, before identity assertion scripts. Native inspections must publish zero further rows. Mixed raw Face paint plus append is prepared in the same owner command; append Undo/Redo prepares target typed Face fields before geometry mutation. The retained post-inspection artifact matched values only after an inspection repaired paint and recorded six publications instead of five; it is an authoritative rejected candidate, not acceptance evidence.

The allocation fixture returns actual allocator null during target vector reservation after VM execution and after the app Undo snapshot is prepared. It requires exact visible Scene, Undo/Redo, revision preservation, controller retirement, charge release and a fresh successful retry. This proves a bounded publication refusal, not a total VM/allocator memory cap.

Costs still open: two O(base) changed-row comparison passes, O(vertices) coordinate index preparation for pose/topology, conversion/cloning of ALL existing Face attrs/layers during mixed append prepare_prefix and append history prepare_snapshot, one full application Scene Undo snapshot, VM root/history snapshots, existing clone allocations and capacity slack outside logical lease accounting. Full Scene/group/global bindings and unrelated application synchronization, stdout/eval/traceback fidelity, arbitrary Python payload projection/callback semantics and total VM/native admission remain required. Default scripting stays disabled; this is an explicitly enabled consumer.

The retained uncaptured eval stdout fixture printed a Face repr before JSON; replacing that fixture expression with assignment keeps the engineering protocol parseable and does not repair or close stdout capture parity.

Zero full final-Mesh export is not a claim of globally O(changed) command execution. The sparse publication metric reports moved/copied final rows; compared_rows and index_vertices expose larger preparation work.

`compared_rows` is the maximum per-kind row extent across the two comparison passes; it is not a count of all comparisons or recursive JSON work.

Focused reproduction (set the environment in provenance.json, -j2):

```
cargo test -p ingetrazo-gui --features engineering-python --lib engineering_python::tests -j2
cargo build -p ingetrazo-gui --features engineering-python --example engineering_console_rows_fixture --example engineering_console_publication_failure --example engineering_console_history_fixture -j2
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/reference-ai-console-rows.py > original.json
$CARGO_TARGET_DIR/debug/examples/engineering_console_rows_fixture > native.json
$CARGO_TARGET_DIR/debug/examples/engineering_console_publication_failure > allocator-refusal.json
python scripts/check-ai-console-rows.py original.json native.json allocator-refusal.json
$CARGO_TARGET_DIR/debug/examples/engineering_console_history_fixture > history.json
python scripts/check-ai-console-history.py docs/audits/ai-console-history/original.json history.json --rows
```

Final named fixture build used the exact frozen source hashes. Eight focused tests ran before the final removal of an empty impl and restoration of untouched test formatting; no semantic changes followed those tests. Both immediate-state fixtures then ran from the final binaries. No full workspace suite or GPU capture was needed for this model-only batch.

The allocator-refusal fixture now directly executes the retained older app Redo and Undo while the VM is retired, checks original paint and mesh/stack states, and confirms charge remains zero before fresh retry. Engine revision=None routes to ordinary saved application history. This bounded document fallback does not preserve old retired VM aliases or prove synchronization of unrelated edits into a live VM.
