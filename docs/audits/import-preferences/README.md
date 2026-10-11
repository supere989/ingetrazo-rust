# Import defaults and unit confirmation

Runtime ba69e2d5 implements actual OBJ/DXF Import unit confirmation and staged preferences. Successful finite proof: 29 typed source/native import phases across eight cases, five Preferences phases and ten rejected checker mutations. The actual egui Modal control passes one test (344 filtered): background CtrlZ blocked; Cancel preserves document/history/settings bytes; real choice and Import produce one Undo/Redo; restart remembers the choice. Named build 6.49s; owner test build 5.07s/run 6.27s.

The original proof invokes the unchanged pinned PreferencesDialog and literal MainWindow `_obj_unit`/`_dxf_unit` methods through actual Qt QInputDialog, original format loaders and SnapshotImport/History. It is a bounded controller/Qt proof, not a full desktop MainWindow run. All raw vertices are compared after group placement, including orphan records. Header/size suggestion precedence, explicit choice, cancellation, persistence before parse and ordinary history remain separate observed phases. Source pin 6be29fe437a7cd992d4e079f71720821b25216b9.

OBJ suggestion uses the literal X-span guard and thresholds 200/20000. DXF uses header plus upper-median size and inclusive 0.05..80 metre range. Parser options scale dimensional coordinates/radii, preserve unitless fields and author only newly emitted points as Qt floats; old unscaled parser callers retain native f64. Open lifecycle is unchanged. Active component Import refuses before reading/mutation in this slice. Invalid native parse preserves document/history while the already accepted choice remains persisted, matching confirmation timing; invalid-file geometry parity is not claimed.

Followups e0475b19 and 795161c0 now pass the [combined root integration](../import-units-integration/README.md) at runtime10013c7c: parser prepare functions return their existing atomic candidate directly to App, removing a redundant whole-Scene wrapper clone; the isolated fixture resets only its own native preference store for deterministic replay. This owner archive and its tested binary remain explicitly ba69e2d5. No duplicate owner rebuild was performed; the final optimized runtime was validated in the single root build and full semantic replay.

A cold parser candidate clone and one App Undo snapshot remain; this is not globally delta-only execution or a heap bound. File bytes belong to the cold dialog and are reused without reread during warm UI. Import preferences expose only operative OBJ/DXF; coordinate mode remains open pending genuine datum entry. Complex unsupported DXF entities/OCS and broad importer completeness remain outside this finite preference workflow.

The first source acquisition API mistake (using nonexistent add_vertex instead of literal source vertex/add_edge) and its isolated settings are retained. No candidate runtime failure or retry occurred. Canonical records.zip authenticates all 18 entries; manifest pins source/runtime files and the exact tested binary. Temporary files remain under /tmp/ingetrazo-import-preferences-proof until root release.

Replay using the existing context_document_lifecycle binary, no new named executable:

```sh
context_document_lifecycle FRESH_REPLAY_DIR --import-preferences
python3 scripts/check-import-preferences.py FRESH_REPLAY_DIR
```

Extract only six OBJ/DXF inputs and original-import-preferences.json for a fresh run; the retained native output/settings/logs are observations, not input authority. The owner test is import_units::tests::actual_import_modal_cancel_accept_and_background_history_ownership. Root can add this mode to the existing context-document acceptance stage without source recapture.
