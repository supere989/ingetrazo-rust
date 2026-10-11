# Composer transaction snapshot optimization

P04 stays Partial. This change snapshots one active sheet lazily at the first item mutation, compares the owned snapshot without cloning either document, and cancels by sheet identity without reverting independent sibling changes. Idle, hover and continuing drag frames do not clone history snapshots. The existing item-rendering clone remains outside this change.

Verify actual egui toolbar, property-menu, text and drag paths plus reordered-sheet cancellation, existing composer tests and retained direct-paper history acceptance. Build the affected GUI tests and consumer once, then execute the resulting binaries without invoking Cargo again.

The original broader pointer-parity capture at /tmp/ingetrazo-composer-transactions-source/source.json was inert: all inputs produced no geometry or history. It is a failed fixture, provides no parity evidence and is preserved. Source script is retained but not run again for this narrower optimization. Unselected dragging, press origin, hidden/locked/z hit testing, snapping, multi-item/resize and remaining tray/script/import transactions stay open. No Python product changes or P04 promotion.
