# Source Scale grip controller foundation

The native controller matches the pinned Python Scale tool exactly for six boxes and 288 grip/modifier combinations: full 3D, flat, line, rotated, reflected-flat and sub-flat. This covers grip counts and order, all box segments, opposite/center anchors, corner/edge/face scaling, uniform toggle, signed point/ray factors and unit-tagged per-axis factors. No tolerance is applied. Four changed inventories are rejected by the checker.

This is an isolated foundation for the full Scale port. The existing application Scale tool has not yet been replaced; groups/images, preview and history, hot retype, snapping ingress and rendered grips remain active implementation work. No feature classification changes from this result.

Original source was captured once with `reference-scale-grips.py`. `records.zip` retains that source, the fresh native output and build logs, including the initial compile failure from the wrong Ray field spelling. Only the affected named consumer rebuilt after correction. Every byte was checked before duplicate output cleanup. Run `python3 scripts/check-scale-grips.py docs/audits/scale-grips`; optionally pass a fresh native JSON output as the second argument. Extract source.json from the archive before running `scale_grips_protocol SOURCE_JSON NATIVE_JSON`.
