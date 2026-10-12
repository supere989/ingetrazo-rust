# Specialized composer workflow inventory

The original `_SheetItem` and specialized classes in `views/composer.py` own pointer gestures. Linear dimensions prioritize separation, endpoint, then label text. Label tips prioritize extra leaders over the primary; Ctrl-tip creates a leader and dropping an extra on the text removes it. Angular and radial handles clamp radius to 2 mm and 1 mm respectively. Labels clamp wrap width to 4 mm. Callouts have a bubble handle and generic box resize. Elevation marks have no specialized handle.

Chain and baseline tools use `_chain_click`, `_seed_run_from`, `finish_chain` and `forget_scene_items`. Runs survive canvas rebuilding and trim live segments on mid-run Undo. Repeated-last, Escape and tool changes finish runs. Only chains with at least two dimensions create a total. Baseline spacing comes from `dimension_style.base_step_mm`, default 8 mm; seeded dimensions retain their axis/offset.

Ctrl selection is intercepted by `_SheetItem.sceneEvent` except label tip Ctrl-copy. Box selection uses window/crossing and modifiers. `note_drag_start` records selected positions; `push_geometry_edit` combines group changes into one history entry. `_panel_edit` propagates only changed fields to same-model-type selected items; `_on_delete_item` removes the full selection. Tests `test_composer_multi_edit.py`, `test_composer_box_select.py`, `test_composer_cota_cadena.py`, `test_cota_linea_base.py` and `test_cota_forzada_recta.py` establish the distinct paths; they are read as source evidence, not rerun here.

The proposed fourteen cohorts are a contract draft pending executable producer review. No capture or build has run.
