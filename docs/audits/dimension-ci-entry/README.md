# Public dimension-anchor CI entry correction

Public run38043968319/job114189643241 failed the local DVec3::X lookup after actual group entry. Unchanged main2f66 reproduces the exact failure; its literal fixture and build/run log are retained. Fresh actual original pinned6be Viewport.begin_group_edit exposes translated WORLD mesh (11,20,30), nested leaf (112,222,333), not local(1,0,0). No production or tolerance change.

5399749 corrects the fixture to move world vertices and create world annotation endpoints. The independent original probe uses operative Viewport begin/end and History MoveVertices/RenameGroup commands. It confirms parent/middle baked transforms survive Undo, leaf placement becomes translation(111,222,333), mesh-shareback Undo preserves the earlier rename, and the next Undo restores the old name. Rust fixture now asserts those literal structural/mesh/name/UID/annotation transitions and redo separately instead of expecting all context entry and previous rename changes to vanish together. Loader mesh-share alias normalization runs before the exact scene Undo baseline; the previous intermediate identity-registry assertion failure is retained.

Fresh immutable source5399749: six original IGZ reopen/live-anchor cases pass, existing12 rotated/reflected/nonuniform dimension-world source controls pass, new two operative original entry/history controls pass. Original probe explicitly rejects the local-coordinate assumption. All source/runtime artifacts remain pinned; no expected tolerance changed.

Additional source boundary discovered and retained: original ReshareInstanceCommand Undo restores mesh but leaves the freshly added dimension in scene (2 annotations before/after Undo). Rust currently atomically restores the full scene, removing it. This repair preserves Rust behavior and records the difference; it does not claim full original annotation/history parity. No rendered label, actual input dispatch or Metal acceptance claim.

Replay from this checkout:

    QT_QPA_PLATFORM=offscreen python scripts/check-python-dimension-anchors.py
    QT_QPA_PLATFORM=offscreen python scripts/check-python-dimension-world.py

Use original Qt6.12 Python environment and isolated CARGO_TARGET_DIR. Checker invokes actual original entry/history probe before native fixture and original reopen checks.
