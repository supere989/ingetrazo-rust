# Operative annotation editing and dimension style

Pinned original: 6be29fe437a7cd992d4e079f71720821b25216b9.

`tools/select.py:330–386` routes double-click glyph labels before geometry; dimensions and leader lines follow picked groups/visible edges. Dialog cancellation and unchanged input add no command. Leader text is stripped, nonempty, and uses EditTextLabelCommand. Dimension text is stripped; empty, `<>`, or the current formatted measured value becomes None. EditDimensionTextCommand and EditTextLabelCommand hold the entity and only replace text, incrementing version on do/undo (`core/history.py:1233–1267`).

`views/tray.py:2354–2457` DimensionStylePanel updates decimals 0–4, units, font 6–28, RGB, arrow/tick/none and baseline spacing 1–60mm directly, preserving other keys. Style is document data but is explicitly nonundoable; live values survive model history and context exit. Native runtime override retains fresh/imported provenance and is explicitly overlaid by the IGZ writer, without inventing imported payload baselines.

Existing accepted Move/Delete/schema and occlusion evidence is reused. This slice does not add annotation Rotate/Scale, renderer changes, or a general platform pixel claim. History still uses cold Scene snapshots; annotation hit layout is cached, while hit testing scans cached rows. Full allocator/VM bounds are not claimed.
