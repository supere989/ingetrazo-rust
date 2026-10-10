# Encoded managed uploads and whole-item canvas compositing

Original source pin: `6be29fe437a7cd992d4e079f71720821b25216b9`.

Raw per-glyph stage `c870f93b` retains the one-pixel original mismatch in each of six declared text regions. The real ash/XTEST replay also passed twelve PDF comparisons; these unchanged PDF results were reused for the focused followup.

Whole-item prototype `ec2216f2` uses source integer premultiplied composition before one managed GPU texture upload. Actual XTEST three-sheet canvas replay and actual original cached QGraphics TextItem comparison produce zero differing pixels in all six unchanged regions. Native binary SHA256: `f2f16abec543320c278b5d9035221c2d1579e1412cbad78cefbd472e323e5f16`.

The focused UI mode does not export PDFs and does not claim dialog proof. Prior full dialog proof remains in the per-glyph evidence. This is experimental text-only coverage; item-cache background/underline, general wrapping/fallback/layout, outer borders and platform rendering remain required. No ROI or tolerance changed.

Vendor focused payload admission test uses an isolated source copy and existing libvulkan.so.1 through an owned temporary linker symlink; no package/host changes. The initial incorrect filter selected zero tests; the retained final log names the actual executed test.
