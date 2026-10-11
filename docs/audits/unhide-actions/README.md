# Hide / Unhide action and history consumer

Frozen Rust cbc7fd24; original pin 6be29fe. The operative original MainWindow action corpus and native IngeTrazoApp consumer match 24 action states plus 10 nested active/parked states. The same validator rejects 19 altered state/inventory controls. The previous saved-view command corpus is replayed from its unchanged original inputs: 60 states, 21 rejected controls. Three core token/incidence and four GUI owner/selection/checkpoint/literal-payload tests pass. No workspace suite or rendered GPU acceptance is claimed.

The Edit submenu routes Selected, Last and All separately. UI Hide filters invisible/no-target selections; raw `hide_selection_command` retains HideCommand semantics for the existing comparator. Last deliberately ignores Face attrs, can alter a detached command-held edge/group, and sees that alteration on its next invocation. Runtime mesh identities survive compaction/snapshot clones and change on recreation/serde reload; semantic equality and disk output omit them. Selection and selection_order remove only actual Hide targets. Detached owners are pruned against both chronological stacks and surviving component cancellation checkpoints.

Mesh snapshots own edge.hidden / face.attrs.hidden; bare AddEdge owns neither. Line snapshot and Delete publication seal their after-state before subsequent presentation changes. Snapshot Redo retains literal JSON hidden payloads, not only truthiness. Derived adjacency is refreshed after Delete without recreating rows: endpoint incidence and unique radial face IDs remain valid after compaction. The original's broader erase/dissolve/merge classifier is outside this cache repair.

Retained failures include the first native snapshot/selection discrepancies and the initial comparator setup using raw DeleteEdgesCommand against UI Delete (which actually uses EraseSelectionCommand). The corrected acquisition uses real MainWindow Delete. The first recreation test found stale endpoint indices; a subsequent serde check required rebuilding the already-existing omitted spatial cache. Neither failure was suppressed. The retained logs also include fixture API compile corrections.

## Reuse / replay

Extract proof.zip to a directory. Run:

```sh
unhide_actions_fixture /path/native.json /path/native-nested.json
python3 scripts/check-unhide-actions.py /path
```

Original source inputs are already frozen in the archive; no reacquisition is necessary. The prior consumer takes its unchanged unique/duplicate input directories and writes JSON to stdout; redirect that output to each native.json before invoking check-saved-view-history.py. Binary and exact source digests are in provenance.json.

## Remaining contracts and costs

This proves the stated action/history corpus, not all model command policies or copy factories. Kept collapse rows must carry source runtime IDs; genuinely recreated rows must receive fresh IDs. Other source transformation/copy factories and shared-prototype editing still require identity-specific auditing. The default history architecture still clones a Scene per command. Visibility tracking adds no second Mesh clone and performs no per-frame work, but its cold action scans, snapshot flag storage, and existing history copies prevent an O(selected) whole-path claim. Rendering/picking/ghost flags are a separate consumer lane; native Metal and full application parity remain open.
