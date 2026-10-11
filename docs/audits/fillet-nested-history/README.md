# Nested Fillet Finish/history correction

Base `54d1153e`, unchanged original `6be29fe4`. The retained source-app bytes
are reused and pinned; no new source acquisition.

The clipboard affine helper cloned all vertices and curve IDs. Original
`Scene.share_back` instead calls `group.transformed_mesh`, reconstructing only
face/edge references then `Mesh.resplit_curves`. A dedicated leaf-shareback
helper now uses the source float/weld reconstruction already implemented for
Change Axes and splits contours at attachments or non-chain vertices. Clipboard
copy policy is unchanged. Candidate preparation and lifted Undo/Redo snapshots
use this same path, so refusal never publishes partial geometry.

All 28 original/native application phases now match at the existing 1e-6 source
coordinate boundary: activation, sizing, refusal, Cancel, commit, Undo/Redo,
child/outer Finish and post-Finish Undo/Redo. The three former Finish/Redo orphan
and curve partition differences are closed with strict vertex counts and curve
bijections, without the historical exceptions. Four actual validation mutations
and the literal previous native candidate reject. One focused core test proves
orphan removal, attached-curve splitting and invalid transform input preservation.
Protected nested metadata variants and broader P09 geometry/rendering remain
outside this bounded correction, as in the original audit; no full P09 claim.

Cold reconstruction allocates a new mesh and curve adjacency/chain sets; existing
Scene history snapshots remain. No per-frame work or total memory bound is added.
New source-global curve numbers are compared by equivalence partitions, matching
the prior criterion; this does not claim global counter numbering equivalence.

Replay frozen named binary with `source.json` and output `native.json`, then:
`python3 scripts/check-fillet-nested-history.py /tmp/fillet-nested-proof`.
`records.zip` retains source, new/old native candidates, focused/build/runtime
logs. `provenance.json` pins every payload and runtime/checker blob. No broad
suite, renderer run, push or tracker edit.
