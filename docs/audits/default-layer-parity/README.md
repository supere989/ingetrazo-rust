# Native default layer and authored tag names

Pinned source: `6be29fe437a7cd992d4e079f71720821b25216b9`.
Fresh original Scene uses `Layer 0`; a default-only IGZ omits its registry. Native previously created `Layer0`. This stage corrects the default constant, omitted/empty registry handling, insertion of a missing default in nonempty registries, exact default visibility, and default assignment clearing. Authored `Layer0`, `Untagged` and other names are preserved, not renamed into the default. Only the exact default name remains protected by layer rename/delete/purge.

`reference-default-layers.py` invokes the actual frozen reader and AssignLayerCommand/History. `default_layer_acceptance` invokes the native reader and app assignment/Undo/Redo. Seven codec cases and five history states compare exactly; the source reader also reopens all twelve fresh native files exactly. `check-default-layers.py` rejects five mutations using the same validator.

The first raw fixture placed `layers` outside the `scene` envelope. Both readers ignored those authored records. The `rename-authored` negative caught the meaningless corpus; that first output is retained in `baseline/invalid-envelope-*`. The corrected source creates format-2 archives with layers at `scene.layers`. No authorship/visibility acceptance is based on the first fixture.

This is a named default/registry/history slice, not general malformed Layer.from_dict coercion or rendered-layer acceptance. No live user documents/config were opened or changed.

Reproduce inside the devbox:
```
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/reference-default-layers.py "$fixture_dir" > "$fixture_dir/original.json"
cargo run -p ingetrazo-gui --example default_layer_acceptance -- "$fixture_dir" > "$fixture_dir/native.json"
python scripts/check-default-layers.py "$fixture_dir"
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/check-python-default-layer-reopen.py "$fixture_dir"
```
Use a fresh temporary fixture directory and purge it after retaining evidence.
