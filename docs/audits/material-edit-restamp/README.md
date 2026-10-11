# Material edit and restamp

Runtime c6582673; named consumers and original recipe script 1da9482e. Four focused GUI tests pass, including atomic refusal and a real Rectangle + restamp → placed component Finish → Undo/Redo restoring both geometry and global paint.

The frozen original has 25 states across root, classic, placed and nested contexts. 23 match exactly. Two post-Finish Undo states retain the original detached-target defect: original shared faces remain recolored; native restores all affected owners and context-specific UVW. These are explicit integrity divergences, not claimed literal parity. Full payloads are retained.

The five-phase tray consumer sends actual egui pointer events to production App.update_ui, cancels then applies removal of an existing texture, and verifies one transaction, selection and revision ownership through Undo/Redo. Closing the modal requests one retirement frame. This proves in-process egui ingress, not OS dialog or GPU pixels.

All five actual App-saved IGZ files reopen using the pinned Python loader. Modeled registry/front/back/IFC fields match the original final states exactly. The original loader drops unknown per-face vendor fields; the raw native archive retains them and the reader script checks that separately. No new texture decoding/mip claim is made.

Replay from repository root:

```sh
python3 scripts/check-material-restamp.py
material_restamp_workflow /tmp/material-restamp-candidate
material_tray_egui /tmp/material-restamp-candidate
python3 scripts/check-material-restamp.py --native /tmp/material-restamp-candidate/native.json --tray /tmp/material-restamp-candidate/tray.json
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/reopen-material-restamp.py /tmp/material-restamp-candidate /tmp/material-restamp-candidate/reopened.json
```

The checker pins literal case/phase inventory, exact payloads, optional recipe removal, paint/opaque ownership and rejects six corruption controls. records.zip includes source/native/tray/original-reader JSON, five saved archives and focused failure/pass logs. provenance.json authenticates archive members and both binaries.

Remaining scope: arbitrary UV/projection editor UI, interactive back-side painting/inference, lazy library registration, native save-dialog gestures, complex topology-changing mixed history and context Cancel of global material commands are not closed by this batch. The mixed Rectangle adversary validates the folded transaction without claiming arbitrary topology equivalence. Full P14 disposition belongs to integration review.
