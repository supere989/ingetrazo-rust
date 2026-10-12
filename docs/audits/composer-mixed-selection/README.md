# Mixed sheet selection source cohort

Frozen source: eight cases, 105 states, eight actual original-reader saves. Three capture attempts were needed; original-mixed.json preserves attempt provenance. The same-kind case was repeated after fixture output metadata failed; successful earlier cohorts were reused.

Replay: `composer_mixed_workflows SOURCE NATIVE`, then pinned Python `scripts/reference-composer-mixed-reopens.py SOURCE NATIVE` and `scripts/check-composer-mixed.py SOURCE NATIVE COMPARISON`. Reader output is native-reopens.json; consumer output is native-mixed.json. Strict comparison retains every geometry/history discrepancy; traversal order is diagnostic.

Qt v6.12.0 source: [QGraphicsItem mouse move](https://github.com/qt/qtbase/blob/v6.12.0/src/widgets/graphicsview/qgraphicsitem.cpp#L6512), lines 6519 onward, takes scene selectedItems for sequential movement. [QGraphicsScene selection](https://github.com/qt/qtbase/blob/v6.12.0/src/widgets/graphicsview/qgraphicsscene.cpp#L1954), lines 1954–1972, declares no particular order and returns the pointer QSet values. Sequential sibling snapping consequently depends on pointer traversal, whereas native uses retained acquisition order with pressed owner first. Frozen mixed group frame Y47.90222222222222 versus native47 and bound-frame image Y43 versus native42.90222222222222 remain strict open deviations. Captured pointer ordering is not a product oracle for a forced native ordering.

All fifteen source categories are represented for selection, movement and deletion. Profile painting is outside this cohort; its semantic bounds and raw preservation are covered. Physical desktop input, arbitrary Qt pointer order, and unobserved heterogeneous handle combinations are not claimed.
