# Active WORLD context snapping axes

Source: pinned original 6be29fe437a7cd992d4e079f71720821b25216b9, Python 3.13.13. `core.axes.sync/frame_axes`, `core.snap.compute_snap`, and operative Viewport Shift acquisition/release helpers define this stage. `probe-python-component-snap-axes.py` freezes 35 controls across five raw frames: red arrow lock, weak/magnetic inference, world/context origins, held/released Shift. Projection/rays are controlled orthographic callbacks; these are caller/function controls, not rendered gestures.

Production screen snapping synchronizes the runtime frame before inference/Shift capture. Normalized context axes drive locks and screen axis inference. The raw stored affine frame remains unchanged; the original world Origin and camera/ground plane policy remain unchanged. Synchronizing a frame does not delete world-space inference memories. GUI synchronizes before Shift rising-edge acquisition.

`check-component-snap-axes.py` independently builds the thin native protocol, compares kind/axis/position against frozen original output (4e-6 tolerance), and rejects changed kind, moved point, missing frame, and missing case. 35/35 source/native controls pass; the existing ten standalone screen controls also pass. Workspace library/integration tests pass with the two existing ignored clipboard controls.

Limits: no new actual pixel/keyboard acceptance here; legacy ray-only snapping has not been ported to the shared axes policy. This changes the axes used by existing memory inference, not its broader acquisition/order algorithm. Those boundaries remain open. Renderer projection/clipping and persisted axes metadata are unchanged.
