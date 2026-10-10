# Paper precision boundary observation stage

Isolated branch base d73f78b, frozen original 6be29fe437a7cd992d4e079f71720821b25216b9. `probe-python-paper-boundary.py` reads actual original camera_basis/world_mesh for the thirteen frozen Composer scenes. Its JSON retains Qt version, source hashes, world vertex positions and edge endpoints, and camera eye/up/basis. It does not substitute a native geometry oracle.

Native `paper_boundary_fixture` checks all thirteen camera eyes exactly and f64 normalized basis coordinates within 2e-15. A negative control requires at least one camera differ from the unchanged general f64 renderer eye by >1e-7. This tiny numerical bound is for normalized NumPy/glam arithmetic only, not page pixels or integer-affine component geometry.

The new OriginalQtPaperBasis API is not yet consumed by Composer or the HLR solver. General HLR and viewport projection remain unchanged. Mesh transformation compatibility, pole/degenerate held-out controls, explicit cache-policy separation and warm worker replay remain pending. No complete paper precision or general f64 HLR parity claim.

Initial whole-page strict PDF comparisons remain retained: 25/26 at72DPI,24/26 at144DPI,23/26 at300DPI:72/78 exact and six failing page/DPI rows. Prior verbal73/78/five tally was incorrect. Files initial-reader-*.json preserve the original failing comparison before integration.
