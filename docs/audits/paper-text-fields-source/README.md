# Production Text fields: bounded source controls

The production exporter expands original single-pass fields and uses original dataclass value equality to identify the first matching sheet. Eight context pages and five actual ComposerWindow caller controls each match the original whole page exactly at 72, 144 and 300 dpi (39 comparisons). The wrong-index valid-PDF mutation changes 39/119/387 pixels and is rejected.

SourcePaperBorder preserves dormant runtime values through native history/recovery. Actual original source distinguishes a disabled authored white border from defaults, omits it when saving, and restores defaults when reopening. Unchanged foreign archive fields remain preserved by native no-op saves. Native tests cover runtime equality, exact large integers, ignored large opaque metadata, recovery, atomic invalid dimensions, codec and local Undo/Redo.

The real Paper border menu is implemented, but its pointer gesture and dynamic-field saveDialog replay are not covered here. Full source property/canvas rendering, font platforms, and P29 remain open. The retained first failures must not be counted as passes.
