# BIM editing-context acceptance

Seven pinned original recipes / 42 phases match exact oriented rings, holes, all face attributes, parked root/group geometry, takeoff and CSV. Identity transform representation alone is normalized; there is no numeric tolerance. Seven mutations are rejected, including 2^-22 quantity/placement changes. The initial six-recipe comparison remains historical evidence.

Actual original MainWindow and native IngeTrazoApp owned-XTEST runs match ten states for BIM activation/name editing, two Rectangle pointer commits, Undo/Redo, and disabled-mode history inside constructor-entered transformed nested contexts. Six mutated UI reports are rejected. Entry itself is not a pointer gesture; images are retained observations, not a renderer pixel equivalence claim.

Ten focused app tests cover root allocation, parked face/group identity, root-only tag/untag, metadata history, stale context/vertex UID atomic refusal, warm cache reuse and actual atomic CSV output. Final parent IFC-only history correction keeps mesh, transform, child list and definition identity checked. Original manual tag/untag does not add History commands; native metadata undo remains an explicit preexisting integrity behavior discrepancy.

The nonflat Freehand metadata failure is retained in logs. Source busy-plane Rebuild precedes AutoTag and starts graph walks at first inserted vertex. Porting those rules fixes the degenerate first fan probe without changing source PAINT-only new-region ownership. All 22 original draw cases now match BIM and all attributes; prior 21/22 result is preserved in the prior audit. This is bounded evidence, not general geometry-row verification.

Remaining: actual context entry/exit UI routing, mixed active/parked manual tagging, general parked-entity tools, active-context IFC menu/export, root/container local-vs-world quantities outside edit (previously documented behavior), and arbitrary precision/source geometry limits. These checks do not promote full BIM parity.

Replay inside ingetrazo-publish from this worktree, using the existing target with dev/test debug=0 and incremental=0:

```sh
cargo build -j2 -p ingetrazo-gui --example bim_context_acceptance_fixture --example bim_context_ui_fixture
/tmp/ingetrazo-parity-python/bin/python scripts/reference-bim-context.py
$CARGO_TARGET_DIR/debug/examples/bim_context_acceptance_fixture docs/audits/bim-context/native.json
/tmp/ingetrazo-parity-python/bin/python scripts/check-bim-context.py
/tmp/ingetrazo-parity-python/bin/python scripts/check-bim-context-ui.py
INGETRAZO_BIM_ORIGINAL=1 /tmp/ingetrazo-parity-python/bin/python scripts/check-bim-context-ui.py
/tmp/ingetrazo-parity-python/bin/python scripts/check-bim-context-ui-reports.py
cargo test -j2 -p ingetrazo-gui --lib bim_controller::draw_tests
cargo test -j2 -p ingetrazo-core --lib segmented_curve_metadata_tests
/tmp/ingetrazo-parity-python/bin/python scripts/check-python-bim-draw.py
```

The last command defaults to the older audit path; retain new output separately rather than overwriting historical evidence. Current regression22 output is copied here. Hashes and precise compiled-stage boundaries are recorded in provenance.json.
