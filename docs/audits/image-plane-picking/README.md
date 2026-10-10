# Image-plane picking operative source comparison

Pinned Python source 6be29fe437a7cd992d4e079f71720821b25216b9: actual Viewport image picking/current-work-plane, SelectTool and LayersPanel. Native: production Scene picking, SelectTool through app dispatch, viewport work plane and layer methods. Literal 29-case registry mandatory; all match exactly. Workspace: 557 passed, two ignored.

Explicit ray projection supplies the original viewport input: controller/geometry controls, not OS pointer or GPU drawing proof. Hidden/locked/zero-opacity/missing-texture references, unknown/default layer names, last-wins ties, front/back/parallel/degenerate/sheared frames, geometry precedence and modifiers are covered. LayersPanel preserves previously selected images when their layer becomes hidden or locked. None/empty layer resolves only literal `Layer 0`; native `Layer0` default spelling remains a separate parity gap.

Retained failures: alias-negative misidentified unknown default aliases; selection-prune-negative pruned images on layer changes; literal old-no-image-selection fails operative selection, with unavailable new APIs omitted rather than fabricated. Interactive transforms, rectangle selection, snapping borders, GPU image drawing and lifecycle remain open.

Replay:
```
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/probe-python-image-picking.py scripts/image-plane-pick-input.json original.json
CARGO_TARGET_DIR=/tmp/ingetrazo-document-global-contracts-target cargo run -p ingetrazo-gui --example image_plane_picking -- scripts/image-plane-pick-input.json > native.json
python3 scripts/check-image-plane-picking.py original.json native.json
```
