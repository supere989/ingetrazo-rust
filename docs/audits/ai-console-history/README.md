# Persistent console application history

Implementation `d7bbd486`, based on published review `e102339e`, optional `engineering-python` feature only. Default production `run_python` remains disabled.

Actual `IngeTrazoApp::run_script` and application Undo/Redo now recognize runtime-only interpreter session/command-depth ownership. The app reserves the destination stack before stepping the real persistent VM command. Successful restoration swaps its existing document snapshot and acknowledges the new model revision. Scene history markers are serde-skipped and cleared from the live scene; unrelated/retired sessions retain the existing synchronization refusal boundary. No new mesh export or hydration occurs during these console-owned history steps.

`recipes.json` is executed by pinned original `core.ai.run_transactional` + `History` and by the genuine Rust application console fixture. Eleven states cover two commands, repeated Undo/Redo, no-op alias inspections and branch truncation. Executed assertions establish actual held Face/Vertex identity and persistent globals; the observation does not guess identity from packed indexes. All positions, face attrs and stack depths match exactly. Actual counters are three forward native exports, seven VM history steps, final owner charge zero. Ten mutations are submitted through the same strict validator and rejected.

`--missing-history-owner` removes only runtime history routing markers, exposing the previous application-snapshot-only history path. The real console then refuses alias inspection after Undo; the fixture rejects it. The retained log is an executable regression control, not a claim that the complete old source tree was rebuilt.

Focused actual app tests also exhaust the real shared logical owner budget for Undo and Redo. They assert unchanged visible mesh, both application/VM stacks, marker and charge; restoring admission permits retry and preserves actual references. Four initial console tests passed before the final test-only strengthening, and the strengthened refusal test passed separately. No workspace suite was repeated.

## Replay

Inside `ingetrazo-publish`, cwd this worktree:

```sh
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/reference-ai-console-history.py > original.json
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo run -p ingetrazo-gui --features engineering-python --example engineering_console_history_fixture -j2 > native.json
python3 scripts/check-ai-console-history.py original.json native.json
```

Focused refusal check uses `cargo test -p ingetrazo-gui --features engineering-python --lib engineering_python::tests::refused_vm_history -j2` with test debug zero and incremental off. The named binary has no normal CLI arguments.

## Explicit remaining scope

Full Scene/group/global bindings, unrelated external-edit synchronization, component context routing, VM-only command chronology and arbitrary callback semantics remain required. Successful forward publication still performs the admitted cold full-mesh export and existing application full-Scene history snapshot; this is not the final delta architecture. Existing VM command snapshots also retain O(root × history) temporary costs. Application snapshot allocation, native/global heap, compiler/builtin deadlines and complete callback/error rollback are not hard bounded by the logical owner budget. No GPU/Metal or rendered-console proof is claimed by these controller observations.

Original focused-refusal and missing-owner failure logs are retained unchanged in `logs.zip`, verified against `logs-manifest.json`. Root independently refreshed both operative Python and frozen native execution in `root-review/`.
