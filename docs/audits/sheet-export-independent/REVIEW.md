# Independent selected-view DXF review

Reviewed candidate `f6a1b15ac4d51c4baa0dea270d05ccf45501fa64` in a fresh detached worktree `/tmp/ingetrazo-sheet-export-review`, with a separate target directory in that worktree. Original source pin: `6be29fe437a7cd992d4e079f71720821b25216b9`; original interpreter Python 3.13.13, independent reader ezdxf 1.4.4.

The fresh `scripts/check-composer-view-dxf.py` run passed all 52 original HLR/writer versus actual native selected-frame exports. It compares class endpoints in camera-plane metres with tolerance 0.00002 m, across current/saved views and hidden-line toggles. Actual native application methods preserve the scene, reuse worker PIDs and warm-cache jobs, reject missing selections without replacing the destination, and reject directory destinations without modifying the document.

Source review confirms that current frames use live application camera state, saved frames use saved snapshot camera state, line classes are Edge/Profile/Cut/Hidden, hidden layers declare DASHED, and sheet millimetres/frame scale do not multiply model coordinates. Output uses a same-directory exclusive temporary file, flush, sync and rename; error paths remove the temporary file. The auxiliary original/native writer reader also passed, including nonfinite input rejection before bytes and preservation of an existing destination.

Private executable SHA256: `738c7f8e3ad2d1465972cae03f4c44d2aa1a7eceb2622054708ef5c2267ac31c`.
Checker SHA256: `1ebc8bcb61ba004012207c1e66786acb920bc8f0cc98ebc52ccb42356ed791e0`.

Commands (from the fresh worktree):

```
CARGO_TARGET_DIR=/tmp/ingetrazo-sheet-export-review/target QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/check-composer-view-dxf.py
CARGO_TARGET_DIR=/tmp/ingetrazo-sheet-export-review/target QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/check-sheet-view-dxf.py
```

This is bounded source/API/file-reader acceptance. Actual export menu gestures, PDF/image export and general CAD reader compatibility remain open. No production source or canonical checkout was modified for this review. Reports here are fresh copies, preserving candidate evidence paths.
