# Ctrl Push/Pull boundary-domain correction

The native Ctrl guard now validates the extracted noninterior boundary of the changed connected body. Interior faces remain untouched document geometry. This closes a valid source case: Ctrl step +0.5 has fourteen faces/two partitions, volume40.5, zero independent triangle boundary errors and zero source/native spatial difference. The original `solid_report` falsely rejects its interior-only edge even though its native manifold accepts the body.

Live original objects were observed before serialization. Ctrl roof −0.2 and step −0.5 each contain eight triangle edge incidence/orientation errors; original manifold rejects both. Wedge −0.4 passes polygon-edge incidence but contains six triangle orientation-balance errors and original manifold rejects it. Explicit original Orient does not repair any of these. These remain literal-parity gaps/invalid-source negatives, not completed features. Native retains atomic Ctrl refusal. No broadening of topology or spatial tolerance.

The changed valid step matches all face geometry/holes/attributes/interior inventories when interior winding is ignored; its one internal wall winding remains opposite the original. Literal oriented inventory remains false, explicitly retained. Physical boundary shape matches exactly; four removed/reversed/metadata/interior-marker corruptions reject.

Ten focused tool tests pass. Seven actual IngeTrazoApp handler controls pass: prior four controls plus nested transformed/shared Ctrl step500mm, local/global Undo/Redo and Finish propagation; two invalid inward Ctrl active-context refusals preserving Scene/history/cancel with visible error. This is production handler/VCB evidence, not OS pointer or rendered capture acceptance.

Replay from worktree:

```sh
/tmp/ingetrazo-parity-python/bin/python scripts/reference-pushpull-live.py docs/audits/pushpull-ctrl-boundary/live-original.json
/tmp/ingetrazo-metal-world-target/debug/examples/pushpull_gate2_fixture docs/audits/pushpull-gate2/source.json docs/audits/pushpull-ctrl-boundary/native.json
/tmp/ingetrazo-parity-python/bin/python scripts/check-pushpull-live-boundary.py docs/audits/pushpull-ctrl-boundary/live-original.json docs/audits/pushpull-ctrl-boundary/native.json docs/audits/pushpull-ctrl-boundary/comparison.json
/tmp/ingetrazo-metal-world-target/debug/examples/pushpull_gate2_app_fixture docs/audits/pushpull-gate2/source.json docs/audits/pushpull-ctrl-boundary/app.json
```

Source/binary hashes are frozen in provenance.json. Target build settings: debug0, incremental0, jobs2. Initial build failed because a new worktree's submodule directory incorrectly contained the source symlink instead of being that symlink; environment corrected without source changes. Earlier speculative interior-winding restoration was tested, did not resolve literal winding, and was removed. Only the boundary extraction is production source in this batch.
