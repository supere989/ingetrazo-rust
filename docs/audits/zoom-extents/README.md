# Original main-window Zoom Extents

Runtime/consumer/source script `a017a70e`, based on `898fe9b4`; immutable Python oracle `6be29fe437a7cd992d4e079f71720821b25216b9`. Ten actual `MainWindow._on_zoom_extents` sessions operate on real original Scene, Group, OrbitCamera and PhotoMesh objects. Only the viewport repaint method is a counter stub; no displayed window or OS event claim.

The named native consumer calls actual App Open and Zoom Extents. All twenty first/repeated camera states match target, distance, yaw, pitch, perspective and two-point mode exactly, without tolerances. Literal case roster: parallel, perspective, two-point, portrait, hidden/nested reflected placement, empty, orphan-only, figure-only, survey-only and hidden-survey. Geometry/model revision, selection and both history stacks remain unchanged. Invalid camera, nonfinite visible geometry and missing survey bytes refuse without changing the camera/model. Eight empty/truncated/duplicate/state/mode/history/refusal mutations fail the same comparator.

The runtime restores the source margin1.1; traverses visible loose entities and placement vertices without making a merged mesh; includes top-level visible figure bounds; keeps the camera unchanged when no visible geometry exists; falls back to visible PhotoMesh only when editable/figure geometry is absent; and ports eight bounded perspective projection-centering/fitting iterations using existing Qt-f32 camera matrices. It does not modify the general f64 camera or renderer APIs. Command-local bounds accumulate by reference, with stack-only corner/NPY-header storage. Original packed survey vertices are streamed from their retained NPZ resource, without a new dependency, image decode or full geometry allocation. Source vertex dtype/shape, declared bytes, compressed/decompressed budget and finite values are admitted before publication. These are cold commands, not per-frame geometry copies; active-context borrowed document projection can still incur existing cold context reconstruction.

Replays (repository root, named binary already built):

```sh
python3 scripts/check-zoom-extents.py
# Extract all capture/ entries from records.zip to a private directory, strip capture/.
zoom_extents_workflow /tmp/zoom-extents-replay
python3 scripts/check-zoom-extents.py --native /tmp/zoom-extents-replay/native.json
```

Do not regenerate the original for integration: `records.zip` contains all ten original-authored IGZ inputs, original JSON, native JSON and build logs. provenance.json pins every archive member, source files and the binary. Initial source script constructor/import mistakes and refused draft Rust literals are recorded; they do not count as accepted cases. All build/runtime handles are terminal, and retained `/tmp/zoom-extents-proof` inputs await root release.

Finite P16 reconciliation: this proof supplies the previously missing Zoom Extents reference sessions and exercises parallel/perspective/two-point main camera modes. Saved-view restoration is supported by `../saved-view-history/README.md` (60 exact original/App states), `../saved-view-identities/README.md` (eight actual original recalls) and `../python-saved-views-evidence.json`. Cursor pivot is supplied separately by `56d899e2` plus required modal followup `7c4bd85e` and its owner proof. Recommend unchanged finite criterion closure only after independent review and integrated replay of both incoming batches. Desktop wheel translation, retained-depth versus ray-fallback pivot, full platform rendering/release gates and arbitrary camera domains remain separate requirements, not inferred here.
