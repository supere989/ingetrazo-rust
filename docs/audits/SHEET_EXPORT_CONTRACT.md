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

`formats/sheet_dxf.rs` is distinct from 3D model DXF. `check-sheet-view-dxf.py` calls the actual original writer and Rust fixture and reopens both with ezdxf; three entities, four class layers, empty layers, signed/extreme coordinates and hidden dashes match. Nonfinite input is rejected before emitting bytes. GUI composer selection/export wiring reuses persistent `ComposerCompute` and camera/scene helpers. `cargo check -p ingetrazo-gui` passes. Actual composer geometry/UI and transactional I/O acceptance remain pending; do not promote P29 or claim release export readiness.

PDF and image writers are still absent. This contract preserves their complete scope.
