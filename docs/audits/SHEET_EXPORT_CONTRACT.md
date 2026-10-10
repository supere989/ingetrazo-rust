# Original sheet export contract and Rust workstream

Frozen original: `6be29fe437a7cd992d4e079f71720821b25216b9`.
Audit source: `views/composer.py` methods `_on_export_dxf`, `model_view_drawing`, `export_image`, `export_pdf`, `export_all_pdf`, `_paint_sheet`; `formats/dxf_out.py`.

## Required final behavior

- Selected-view DXF uses orthographic camera-plane model metres, no paper scale or crop. Frame-bound scene/camera/section, profiles and hidden-line settings apply without changing the live model. Declare edge, PERFIL and CORTE layers even when empty; declare OCULTAS only when hidden lines enabled. Layer title is the source view name, not custom frame title or scale; source Unicode uppercase then ASCII replacement. R12 LINE coordinates use nine significant digits. Hidden linetype declares 0.5-metre dash / 0.25-metre gap. Preserve source line-class grouping order.
- Single sheet PDF preserves exact physical page size and orientation and uses the same mm-space item painters as preview, not a screenshot of the editor.
- PDF atlas exports all compositions in document order, each with its own page metrics. One failed frame must not abort the atlas: retain an explicit missing-view placeholder and report affected sheet names.
- PNG/JPEG export paints white paper at physical size times DPI/25.4, nearest-even rounded dimensions with minimum one pixel, resolution metadata, antialias/text/smooth image rendering, JPEG quality 92. Default 200 DPI; UI persisted range 50-1200 and inferred extension.
- Export painter supports every original item, sorted by z, skips hidden items, translates item coordinates, expands document/sheet/frame fields, and paints the sheet border last. Includes view frames, images, text, scale bars, terrain profiles, north arrows, legends, shapes, linear/angular/radial dimensions, tags, levels, callouts and title blocks.
- View export refreshes original render/annotation content and preserves live scene, camera, selection/history. Printing/preview optional-support error must remain visible; PDF remains available independent of print module.

## Acceptance evidence still required

Actual original/native selected-view HLR exports for standard/current/saved rotated views, section holes, nested geometry, hidden toggles, profile toggles, custom title versus view title, and extreme coordinate magnitudes. Reopen both DXFs with independent ezdxf and compare all line endpoints, class layers and linetype data; malformed/worker/I/O failures preserve destination and document state. Render a real UI select/export/cancel workflow.

PDF/atlas: independently reopen page count and MediaBox/orientation, physical vector geometry/scale and text; render every page and inspect all item kinds, z/hidden controls, border stacking and missing-frame isolation. Images: independent decoder verifies dimensions/DPI, actual encoded PNG/JPEG, physical geometry and appearance against original. Fresh clean-package exports must work with packaged persistent workers on each claimed platform.

## Current bounded implementation

`formats/sheet_dxf.rs` is distinct from 3D model DXF. `check-sheet-view-dxf.py` calls the actual original writer and Rust fixture and reopens both with ezdxf; three entities, four class layers, empty layers, signed/extreme coordinates and hidden dashes match. Nonfinite input is rejected before emitting bytes. GUI composer selection/export wiring reuses persistent `ComposerCompute` and camera/scene helpers. Canonical production geometry and transactional I/O evidence now covers the 52 cases described below; independent reader results are retained in `sheet-export-independent/` and the fresh canonical rerun in `sheet-export-canonical/`. P29 is Partial. Actual UI export gestures, wider workflows and PDF/image acceptance remain open; this is not release export readiness.

PDF and image writers are still absent. This contract preserves their complete scope.

Updated bounded production evidence: fresh thirteen source HLR/IGZ inputs in both hidden modes reopen as 26 original/native DXFs with matching classified endpoints within 2e-5 metres. Actual application saved-frame export preserves scene, uses two persistent workers and warm cache without jobs; invalid selection and directory I/O failure preserve destination/document. The initial fixture incorrectly captured the untouched application camera rather than loaded source camera; its 26 failures remain retained. Corrected fixture explicitly restores application camera before saved-view capture. UI gestures, wider camera/profile/source naming, Windows replacement and PDF/image remain open.

Current live-camera correction: export now bases current/standard frames on the application camera, and bound frames on their saved scene camera. Fifty-two fresh original/native cases (thirteen scenes × hidden modes × saved/current) pass with deliberate stale document-camera divergence, retained in reader-results.json. This does not yet establish composer preview current-camera fidelity or actual UI export acceptance.
