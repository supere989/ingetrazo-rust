Original direct page/border property history

Original ComposerWindow._on_page_changed, _on_border_changed and _on_pick_border_color mutate the composition and mark dirty, without ComposerHistory commands. Source histories only undo command targets. Native whole-sheet snapshots previously created property commands and restored older border settings during item Undo; the retained historical evidence is paper-text-fields-ui and paper-text-fields-canvas-cache (Undo enabled the border), while actual original empty history kept disabled white.

b1576f3c makes direct page/border UI changes dirty without creating item history. When finishing/replaying item snapshots it retains the current paper/orientation, runtime border/baseline and original native border object. Direct properties do not clear pending item Redo. The Ctrl-event regression now exercises an actual moved item command rather than the formerly divergent border property command.

Five exact original/native controls run operative source handlers with genuine source widget values and AddItemCommand/EditItemCommand: empty history, earlier item Undo with later property edits, direct-property preservation of pending Redo, Undo all items and Redo all items. Source whole-property handlers and actual compiled native methods produce equal states.

Fresh actual original checkbox/QColorDialog/Ctrl-Z preserves disabled white at 0/0 history. Native genuine XTEST properties/Ctrl-Z/Y also preserve it at 0/0; real Add North/Undo/Redo/remove gestures remain correct and preserve the border. Genuine current/all/warm GTK dialogs still produce 12 source-exact PDF comparisons. This resolves page/border property history only; broader source command targeting and Text canvas/font/P29 are not promoted.
