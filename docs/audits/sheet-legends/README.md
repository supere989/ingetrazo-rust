# Physical sheet legends

Imported `leyendas` snapshots now render through the shared physical PDF and direct native PNG producers. Cold borrowed export references preserve source category 5, stable z/order, hidden state, literal snapshot rows and translated empty labels. PNG/PDF preparation admits all inputs before destination publication. Save preserves opaque native legend metadata; the original IGZ reader reacquires the seven snapshots unchanged.

Seven actual pinned Python cases cover English/Spanish empty legends, rows, wrapping, same-z overlap, high-z overlap and hidden legends. Original commit: `6be29fe437a7cd992d4e079f71720821b25216b9`. Source archive is immutable: `source-records.zip` authenticated by `source-manifest.json`. The failed initial helper completed four cases before an invalid TextoItem constructor; the corrected helper reused those four and captured only the three missing cases. Both helpers and failure/recovery receipts are retained in `records.zip`.

Root joint build at `3a43b9e9d504311008607ce43b37e200d2d0bf37` took 12.103597 seconds. Binary SHA256: `26321c3fb25af1e079cb49dd1c6afd67c5eacd503f01edbf46a064376cef1b6f`. Software Vulkan and one actual NVIDIA RTX 5090 execution produce identical whole-image hashes: **6/7 PNG exact, 7/7 PDF exact, seven original-reader roundtrips and ten negative controls**. Strict checker exits 1. `legend-same-z` retains one pixel at (255,212): source (84,86,92,255), native (84,86,91,255). Cause is unproved; no tolerance, pixel override or fitted arithmetic was added. Native runtime returns 0 and GPU ledger drains. Private CDI bundle is unchanged, authenticated in the transport receipt.

Replay from the integrated repository with the pinned Qt environment:

```sh
/tmp/ingetrazo-parity-python/bin/python scripts/replay-sheet-legends.py /ABS/debug/examples/sheet_legend_workflows /FRESH/output --software
```

Actual GPU transport is `scripts/run-sheet-legends-nvidia.py`; its fixed paths bind this historical integrated binary/source and retained private CDI, and its complete command is retained in `nvidia-3a43/transport.json`. Compare output with the exact compiled checker archived under `compiled-scripts`.

This is partial physical export acceptance, not full P29 completion. Legend editing/refresh UI, arbitrary unsupported sheet painters and macOS font/pixel parity are outside this proof. No old accepted image/symbol/caption corpus was recaptured or rerun by the owner. Root independently ran affected regressions in its shared build.
