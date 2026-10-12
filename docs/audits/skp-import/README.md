# Native SketchUp App import workflow

This packet is a build-ready protocol, not acceptance evidence. The original producer and native consumer each perform actual chooser-provider → parser/worker → App publication → Undo/Redo → native/original save-reader workflows. Capture/build execution belongs to the integration lane.

The finite roster is 15 cases, 71 states and 12 saved original-reader pairs. It reuses hash-bound engineering SKP files already retained by parser proofs, adds camera records using the pinned OpenSKP TLV grammar, and authors one modern empty record/flat-thumbnail control. This does not claim broad proprietary version or real-user-file coverage.

Accepted source behavior includes document-root groups during component edit, local/axes matrices and relational prototype sharing, named material collision/reuse, material/back-face color persistence through Undo, layer visibility, perspective/parallel camera views and hidden layers, existing-name preservation, dimensions and leader text. The nested witness includes a real prior edge/label command and finished Undo/Redo. Source absent matrices are observed as identity values; directed face rings and meaningful matrix coefficients remain exact.

The native file worker holds only a private file-derived plan. It does not receive or clone a live document. Publication pre-admits candidates and history/checkpoint slots; no units/options dialog or camera fit is invented. The progress modal has no user Cancel, matching the original. Unrecognised/unreadable files use the original warning outcome; there is no external converter fallback. Unsupported lossy records and decoder/inventory admissions remain explicit refusals.

The general replacement-loader policy remains unchanged, including its saved-scene and empty-document refusal. The explicit App policy consumes the decoded saved-camera records and admits only source-verified modern empty files. No viewport/GPU pixels, memory benchmark, whole-process memory cap or desktop file-picker automation is claimed. Controlled chooser subprocesses and actual egui modal clicks are declared ingress.

Run the original producer once after independent review:

```sh
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/reference-skp-import.py /absolute/source-output
```

Run the prebuilt native consumer as `skp_import_workflows SOURCE NATIVE`, then `reference-skp-import-reopens.py SOURCE NATIVE` using the pinned original Qt reader, and `check-skp-import.py SOURCE NATIVE COMPARISON.json`. The checker compares full finite state and all saved-reader observations, with exact numeric camera distance allowing only lossless positive finite Python int/native-f64 representation.

Original source is pinned at `6be29fe437a7cd992d4e079f71720821b25216b9`; OpenSKP at `291700bc213655a00461838de71e5f206311e619`. The contract authenticates actual original files, reused inputs, producer/observer/AST seed helper and parser module bytes before Qt setup. Original `_parse_pages`: `packages/python/src/openskp/_core.py:757–829`; original App import: `views/main_window.py:4994–5058`; merge: `formats/skp.py:161–380`; material collision: `core/materials.py:120–140`; empty witness: `formats/skp_openskp.py:882–916`.
