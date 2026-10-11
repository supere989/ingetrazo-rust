# BIM tag-as-you-draw handoff

Runtime source **3501a8b7**, based on published **77027f5d**. Original pin **6be29fe437a7cd992d4e079f71720821b25216b9**. No broad suite, cloud run, original-source or canonical tracker edit.

The native plan carries explicit authored AddFace identities before coverage reconstruction. Auto chord halves/cycles get one fresh ID per trace; re-emitted/carved neighboring regions do not. Inherited truthy IFC survives. Central JSON truthiness matches Python; malformed truthy nonobject IFC refuses before overwrite, with signed-ID exhaustion/range refusal. Circle/Polygon/direct Arc report their literal face commands. Flat Freehand/Arc rebuild-only faces remain untagged, as in original commit_arc.

The real BIM checkbox publishes runtime Scene.active_ifc (serde-skip); class changes rearm immediately and names rearm on blur/Enter, matching QLineEdit.editingFinished. Successful New/Open clear activation; ordinary model history preserves the current mode. Active group/component draw contexts refuse before mutation and remain OPEN. PushPull extends an existing base tag independently of the active mode. Shared app transactions retain Undo/Redo and failure safety.

## Evidence

- Original BIM regression 7/7; focused native app checks 7/7; JSON truthiness/refusal checks 3/3. Logs retained in logs/.
- report.json: 22/22 exact oriented rings/holes and BIM tags, 21/22 all attributes. First 17 mandatory cases match all attributes. Extra nonflat segmented Freehand chord keeps vendor=71 on both native cells; original keeps it only on the upper cell. This general drafting metadata discrepancy remains OPEN, not a full-attribute pass. results.json freezes complete outputs.
- ui-comparison.json: 10 actual source/native UI phases match class/name/object IDs, face census, activation and history; six mutations reject. XTEST drives production IngeTrazoApp update and unmodified original MainWindow/BimPanel: checkbox, two rectangles, active-name edit/finish, Undo/Redo, off, off-Undo/Redo. PNGs under ui/ and original-ui/. Default Glow software UI; no native Vulkan/Metal pixel claim. Different viewport sizes/pointer quantization mean UI rings are not compared; the command corpus separately checks exact rings.
- Original Ctrl+Shift+Z fails due duplicate QAction shortcut ambiguity; original-shortcut-* retains the failure. Actual source Edit → Redo menu succeeds. Existing native shortcut behavior is unchanged.
- initial-report.json/initial-results.json retain two early flat-curve tagging failures. source-name-editing-finished-initial-* retains the premature pre-finish source observation. Initial exact analytic volume expectation 2.0 was wrong: actual original and native both produce 1.9999999999999998 for the raised 2×1×1 rectangle. No production precision/tolerance changed.

## Replay

Inside ingetrazo-publish, cwd this checkout, with ingetrazo symlink to /work/ingetrazo:

    export CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target
    export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
    cargo test --locked -p ingetrazo-core --lib drawn_truthiness_tests -j2
    cargo test --locked -p ingetrazo-gui --lib bim_controller::draw_tests -j2
    /tmp/ingetrazo-parity-python/bin/python scripts/check-python-bim-draw.py
    cargo build --locked -p ingetrazo-gui --example bim_draw_ui_fixture -j2
    /tmp/ingetrazo-parity-python/bin/python scripts/check-bim-draw-ui.py
    INGETRAZO_BIM_ORIGINAL=1 /tmp/ingetrazo-parity-python/bin/python scripts/check-bim-draw-ui.py
    /tmp/ingetrazo-parity-python/bin/python scripts/check-bim-draw-ui-reports.py

The command checker reports bim_contract_passed=true and full_attributes_passed=false separately; it requires first-17 full comparisons plus all-22 BIM comparisons, retaining the extra attribute failure. No full drafting claim.

manifest.json pins production/fixture hashes. Native command binary c53a8aca911af3350887f3e05c270331995b2ae605da51a1661f14d8f967e76f; delivered UI binary 24ebd7499e7442e00b3c5924557fd0c852954b4e4462ba9c6805a538f3ee17ed. Stable build cache remains owned for ongoing work; terminal outputs and mutable observation state are purged after retention.

General solid precision, active-edit contexts, broader drafting/Arc geometry, opaque metadata, renderer/platform parity and all full-row gates remain OPEN.
