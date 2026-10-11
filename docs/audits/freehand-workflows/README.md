# P06 Freehand and legacy drafting Repeat closure

The retained source pin is `6be29fe437a7cd992d4e079f71720821b25216b9`. All 15 recipes and 152 phases match the actual native App consumer; three native egui sessions and 11 meaningful corruptions pass. Ten affected Rectangle and two Freehand unit tests pass. The shared curve correction also passes the retained combined 66-shape/488-state and 22 drafting comparisons without recapturing their originals.

Freehand now follows the source unsnapped ingress, Qt-authored stroke points, and flat curve-resplitting law. Line, Rectangle and Freehand retain their drafting identity through Select and recreate the correct controller through Repeat Last Command. Rectangle preview preserves the source zero-width sides and 22-pixel cursor square after commit/Cancel, while commit and VCB still reject degenerate geometry.

`records.zip` retains all first failures, corrected records, logs and their manifest. The first three-repeat fixture used inconsistent live-camera projection with explicit 100 px/world recipes; its 142/152 result remains visible. Only those three missing sessions were corrected and captured again. Existing 12 Freehand, 66 shape and 22 drafting sources remain unchanged.

Replay the prebuilt consumer after extracting the canonical archive:

```sh
/tmp/ingetrazo-native-font-merge-target/debug/examples/freehand_workflows /tmp/freehand-records/source-combined.json /tmp/freehand-native.json
python3 scripts/check-freehand-workflows.py /tmp/freehand-records/source-combined.json /tmp/freehand-native.json /tmp/freehand-check.json
```

A named build uses `CARGO_TARGET_DIR=/tmp/ingetrazo-native-font-merge-target CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build --locked -p ingetrazo-gui --example freehand_workflows`. Source capture uses `/tmp/ingetrazo-parity-python/bin/python scripts/reference-freehand-workflows.py OUTPUT`; `--repeat-command` captures only the three legacy Repeat addendum cases. Reuse canonical sources for acceptance rather than recapturing.

The finite criterion mapping is in `criterion-review.json`. It recommends P06 satisfaction conditional on the integrated named replay. Full desktop/overlay pixels, broader P07 inference, raw entity identity and all other tools' Repeat contracts are separate. No tracker promotion or production rendering claim is made here.
