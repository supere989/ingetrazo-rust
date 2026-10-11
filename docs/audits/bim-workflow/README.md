# BIM editing-context IFC workflow

Runtime source: `47d7e952` plus the missing-vertex refusal-only followup recorded in provenance, based on published `aaea1a31`. Original source pin: `6be29fe437a7cd992d4e079f71720821b25216b9`.

The actual pointer protocol has **11 phases**: initial, parent-entry, child-entry, active, drawn, undo, redo, csv, file-menu, export-menu, ifc. Both applications enter the translated parent and child through actual double clicks, enable BIM drawing, commit Rectangle, deliver Undo/Redo, save CSV through a genuine native save dialog while both contexts remain active, then save IFC through the genuine dialog after both contexts finish. Native uses the production default Glow application host, original uses its real XCB MainWindow. This is not a Metal/Vulkan pixel acceptance claim.

Operative source `MainWindow._on_export_ifc` first calls `Viewport.end_group_edit`, before census or dialog; that method leaves every context. Source CSV leaves contexts active. Source keeps Rectangle after exit. The first retained native run incorrectly changed it to Select (`initial-ui` versus `initial-original-ui`); the Rectangle global-preview exit contract corrects that discrepancy. Initial build failure was an empty original gitlink/resource symlink setup failure, not product runtime behavior.

Independent IfcOpenShell 0.9.0 parses both genuinely saved files, runs EXPRESS validation with zero violations, and compares exact root slab type/name, oriented ring and quantities. Name, class and coordinate mutations are rejected. The untyped nested groups' internally drawn tags do not become census objects in the original; the exported object is the parked root slab. Pointer-authored active geometry differs at subpixel quantization, so this UI proof does not claim exact active rectangle coordinates. Exact command geometry has prior source fixtures.

Mixed active/parked manual tagging is permitted by source `_on_tag`. Native now resolves immutable context paths and face vertex identities before publication, preserves both domains, and routes one metadata patch through Undo/Redo and ordinary/layer shareback Finish. Four focused controls pass: mixed ordinary history across Finish, stale target atomic refusal, nested placed-leaf folded geometry plus root metadata Undo/Redo, and outside-history metadata drift refusal. Missing faces removed by source whole-prototype Undo are allowed; ambiguous identities are refused. Active placement previews visibly refuse mixed metadata mutation/history rather than risking a preview overwrite.

**Remaining:** actual pointer mixed-tag delivery and cancellation-dialog capture are not proved here. Original manual tagging creates no History command; native adds an undoable metadata command, an explicit prior policy discrepancy. Interleaved unrelated root metadata commands around shared leaf folds and total metadata-history byte budgeting remain unproved. Source local-shape versus native world-shape IFC quantities remain the prior documented discrepancy for transformed tagged groups. Full BIM/general solid precision parity is open. This batch does not promote a tracker row.

Replay:

```sh
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -j2 -p ingetrazo-gui --lib bim_controller::draw_tests::mixed
# IFC focused test filter: bim_controller::draw_tests::ifc
cargo build -j2 -p ingetrazo-gui --example bim_workflow_ui_fixture
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target /tmp/ingetrazo-parity-python/bin/python scripts/check-bim-workflow-ui.py
INGETRAZO_BIM_ORIGINAL=1 CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target /tmp/ingetrazo-parity-python/bin/python scripts/check-bim-workflow-ui.py
/tmp/ingetrazo-ifc-validation-python/bin/python scripts/check-bim-workflow-ifc.py
```

All commands execute inside `ingetrazo-publish`. Fixtures observe state only; pointer/key delivery uses an owned Xvfb/XTEST process, and actual Qt/Zenity dialogs choose the destination. No mocked exporter/dialog backend.
