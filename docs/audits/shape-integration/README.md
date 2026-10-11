# Integrated drafting and document acceptance

The integrated Rust tree combines published appearance/nested-import `3195c7b6` with shape runtime `313c287c`. Independent owner reviews are retained separately. This combination changes the complete Rust tree, so three named consumers were built together once after retiring only local-package target artifacts; external dependency artifacts were reused.

The build took 10.58 seconds. A two-process affected replay took 12.61 seconds: all 66 shape recipes/488 phases and seven actual egui controls pass, as do the retained 22 drafting recipes, nested import with fifteen original-reader reopens, unchanged import preferences and appearance. The three frozen shape cohorts are combined without changing their rows, so the same seven egui recipes execute once. No original handler was recaptured and no broad workspace suite was repeated locally.

`manifest.json` authenticates every archive entry. `records.zip/provenance.json` binds the exact integrated Rust tree, lockfile, named binaries, source archives and checkers; build and per-stage timings, original inputs, actual native observations, fifteen reopened saved-file comparisons and acceptance reports are retained. Every archived member was byte-compared with its raw producer file before freezing.

P06 stays Implemented pending the distinct Freehand handler sampling/simplification/cancel/repeat proof. Full desktop/renderer behavior and all release gates remain under their existing criteria. The next stable combined publication owns broad CI; this receipt does not claim unpublished CI success.
