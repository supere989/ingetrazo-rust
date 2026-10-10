# Independent CAD callback boundary review

Reviewed source f71e2c93 adds a static composer backend enum. Linux/Windows dispatch to the existing Vulkan handles and shaders; Metal producers are not installed in the app at this checkpoint. Frozen snapshots retain immutable Arc geometry and palette, pose and revision. Unchanged updates keep their revision and do not scan or copy geometry. Native consumers must retain the snapshot until real completion.

Root strengthened controls cover invalid index ranges and NaN palette rejection without changing geometry, palette, pose or revision; a valid palette update preserves the earlier frozen owner. Removing changed-palette admission makes the new assertion fail; restoring source passes. Six existing composer cache/geometry tests and both Linux fixtures pass; GUI library cross-checks pass on both Apple architectures.

The first full workspace build exposed an omitted sheet-shape Vulkan factory conversion. That E0308 log is retained. The reviewed conversion preserves the same Vulkan factory through the enum, and the fresh combined workspace passes 543 tests, two ignored. This is producer ownership and compile integration evidence. Actual Metal callback consumption, GPU accounting, source image parity, warm uploads and physical Mac acceptance remain OPEN. The isolated Metal host is not adopted by this change.
