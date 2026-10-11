# Batched stencil checkpoint

Actual NVIDIA replay at 1df258189ad55d79e68334cd773b92d2354f663a: all 28 complete images exact against unchanged pinned Qt references; allocation denial preserves the old image, restored-budget retry and resize return the original image, final GPU ledger zero. Ellipse-alpha marker/composite draws fall from 198 to 4 per frame; this is a draw-count result, not a GPU timing claim. Packed bytes increase from 14,256 to 44,544.

Later source-key correction 50c66a0f4b288bd3ad8cd21327766a621ad0125f has focused same-geometry changed-triangle admission/packed-byte proof in `source-key-test.log` (retained byte-for-byte in `focused-test-log.zip`). It also keys batch layout, while visibility and color-only changes reuse packed records. The earlier GPU replay is not relabeled as this revision.

Non-default probe only. Tighter conservative coverage, measured fragment cost/throughput, wider pose/DPR/application acceptance remain open.

Run scripts/check-annotation-stencil-batched.py with Python, Pillow and NumPy. It verifies archive/member hashes, mandatory inventory, full images, warm counters, refusal/resize images, draw counts and actual altered-pixel/missing-case negatives.
