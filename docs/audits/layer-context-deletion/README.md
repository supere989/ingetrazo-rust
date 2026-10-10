# Layer deletion during leaf and nested edit

Pinned operative original LayersPanel deletion traverses the loose document and
all actual placed meshes, even while a leaf edit is open. It does not mutate a
hidden private prototype merely because an edit session holds it. A visible
sibling holding that same prototype does mutate it; Reshare captures its current
state when the command is created on Finish.

Four actual original/native controller cases (single, linked, nested single,
nested linked), 16 states, three rejected ownership corruptions. Native entry
observes active mesh/layers/history only; parked outside/sibling entry fields are
retained in original but not claimed as observed native values. Finished states,
Undo and Redo compare all recorded fields, with explicit default Layer0 spelling
normalization. Source single Undo restores Tagged on the inner face while the
removed registry/outside edge remain default. Linked Undo keeps the shared face
untagged because the original prototype changed through its visible sibling.

The literal pre-fix native replay remains `baseline-native.json`: outer tags
survived Finish incorrectly. The checker rejects that actual recorded value,
independently of its missing nested cases. An intermediate linked cache failure
was also observed before synchronizing active registry caches with the changed
parked prototypes. No tolerance or production transform policy changed.

Implementation applies deletion to actual parked document owners, excludes
hidden edit prototypes, updates known shared-mesh aliases and definition caches,
and keeps cancel-scene global deletion state. This is a cold layer operation;
no new per-frame copying, worker or GPU path. Private baseline mesh snapshots
remain available for source-specific Undo. Current-source full workspace:
540 passed/2 ignored. Prior four layer ownership cases still pass.

A distinct original edge-only boundary is retained: source transformed_mesh
omits edge-layer values, so edge-only leaf entry already loses its edit copy tag
and Finish can restore the original tag without recording a shareback command.
This stage's accepted corpus uses face attributes that original preserves.
Native edge metadata preservation has not been weakened to fit this source
behavior; full edge-copy/history parity remains explicitly unaccepted pending
review. Reproduce the retained probe with PYTHONPATH=scripts and Qt offscreen.

No full layer/P15 closure: rendered input, all command-specific attribute owner
policies, broader annotation families, edit cancellation and file save/reopen
of these deletion cases remain additional acceptance gates. Image planes/style/
shadows/richer settings and tile capture remain separate full-scope work.
