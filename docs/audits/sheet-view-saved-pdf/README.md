# Saved-view vector paper adapter acceptance

Replay in an isolated worktree/target:

```
INGETRAZO_VIEW_PDF_SAVED=1 CARGO_TARGET_DIR=<private-target> QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/check-composer-vector-view-pdf.py <new-artifact-directory>
```

Original `SavedView.capture/apply`, `apply_frame_camera`, `Viewport.hlr_geometry`, and actual `ComposerWindow.export_all_pdf` are operative. Saved layer/object/cut state is captured first; current groups/layers are then hidden and current cuts disabled before IGZ freeze. Source saved hidden geometry/object ghost toggles are true. The source painter restores saved visibility/cuts; native uses a borrowed view adapter. Original camera target/up/basis observations remain frozen separately. Explicit hidden-layer source controls retain hidden geometry rather than unhide all entities.

Native model equality is checked after export. Borrowed nested hidden IDs, unknown layer visibility, explicit alternate cut plane, no-cut and cuts-off states have unit controls against the independent core saved-application API. Actual original pages cover captured cuts and live cut suppression, reflected/nested placement, soft silhouettes, known hidden group/face layers. Alternate-cut/off-state unit controls are not promoted to original whole-page proof.

Source `hidden_shown` only affects ghost drawing/selectability, not `entity_visible`/paper ink. Native changes to those flags reuse the same completed record. Altering saved hidden layers at unchanged document/model revisions publishes new geometry; restoration reuses the prior completed record. The material is cached with ink and counted against the existing memory budget, avoiding cap triangulation solely for warm fill admission.

Remaining P29 work includes source solid/hatch cut fills, grids/annotations, Text/title/scale/empty-frame placeholder, perspective, original raster/default style using direct native GPU offscreen embedding, further camera/section precision controls and actual View dialog gestures. No full-row promotion.
