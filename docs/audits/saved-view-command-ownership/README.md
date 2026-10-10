# Saved-view command ownership

Source620054c, actual original pinned6be29fe437a7cd992d4e079f71720821b25216b9. Fresh original ScenesPanel add/update/rename/delete uses operative panel handlers, Viewport edit entry/Finish and command History. Native app-method workflow mirrors model operations. No rendered mouse/keyboard or complete history claim.

Original saved-view panel edits are not model commands. Rust now seals each model command's added/removed view membership delta BEFORE a non-command panel mutation. Ordinary restore applies only that command delta to the live view list. Original SnapshotImport holds imported objects; runtime identity and delta-held object refresh let a renamed then deleted imported view reappear renamed on importRedo. A new UI view on the undone branch preserves Redo and remains first; importRedo appends its own view, exactly source. Identity/delta metadata never enters IGZ. Imported membership remains undoable; lists are not blanket excluded.

Linear set lookup at cold command boundaries avoids quadratic membership scanning. UI recapture/rename refresh identity-held values in pending command deltas, matching source reference lifetimes. Empty sealed metadata is semantically equal to an unsealed Scene snapshot, while nonempty ownership remains compared; no model change follows from recording an empty seal. Finish prepares seal in staged history copies so refused candidate/invalid historical mesh cannot mutate live stacks. Initial refusal regression retained; current full test suite passes.

Bounded pass: savedview/path/sheet ordinary/folded exact state contents and correct2/3command counts; four import-object lifetime states exact; four payload negatives and literal old extra-history failure retained in prior presentation-history audit. Existing guide/text/geopoint/datum20states+4entries and annotation8states/4negative replay pass. Native imported-view addition is exercised as the same app snapshot/add operation boundary rather than a rendered file-import menu; original uses real SnapshotImport.

Explicit OPEN: ordinary terrain visibility still restores stale values; verdict records false. No all-presentation/P15/Metal closure. Original source image planes/style/shadows/richer units/cutlist still need modeled operative contracts. Runtime Scene serialization/reload resets command metadata and runtime identities, matching fresh-document history reset; cross-restart undo persistence is not claimed.

Replay:

```
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/probe-python-presentation-history.py /tmp/original-membership.json
CARGO_TARGET_DIR=/tmp/isolated-membership-target cargo run --locked -p ingetrazo-gui --example presentation_history_fixture -- /tmp/native-membership
python3 scripts/check-saved-view-membership.py /tmp/original-membership.json /tmp/native-membership/native.json docs/audits/presentation-history/native-baseline.json
```
