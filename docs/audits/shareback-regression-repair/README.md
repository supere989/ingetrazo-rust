# Shareback regression repair

Base bf85b4c4. Published Linux job114489360992 exposed three unchanged strict
regressions: mixed BIM metadata Undo after Finish, f64 rotated/scaled dimension
length after Finish, and malformed unreferenced infinite vertex Finish refusal.
The first was independently reproduced locally; its initial failure log remains.

Source reconstruction stays in place: referenced entities rebuild, unused finite
vertices disappear, and curve contours split. Its native shareback policy now
retains vertex UUIDs and complete native Face/Edge metadata/history tokens using
input-row correspondence, rebuilding derived adjacency rather than copying stale
indices. Incompatible metadata on merged edges refuses before publication.
Change Axes retains its separate source policy (fresh reconstructed entity IDs).
All input vertices, even unused ones, preflight for finiteness; references and
computed inverse also validate before candidates can publish.

Source QMatrix4x4 frames are exactly f32-representable. For such shareback frames
retain the Qt mapping law; native-authored f64 frames that cannot roundtrip f32
use f64 mapping/inverse so existing world measurements are not narrowed. This is
a native precision-preservation boundary, not a changed source coordinate tolerance.

The exact three failed tests pass unchanged. Directly affected core reconstruction
and annotation-history tests also pass (five focused tests total). Protected-record
control verifies opaque face/edge data, interior/layers/hidden flags and tokens;
malformed orphan, incompatible merged metadata and invalid transform refuse;
f64 mapping and survivor UUID correspondence are checked directly.

One joint named build replays unchanged original inputs: 28 exact Fillet app
phases and33 Change Axes app states; four Fillet mutations+oldcandidate and eight
Axes mutations+three retained failures reject. The five saved Axes IGZ bytes remain
identical to existing actual Python reader inputs, so no reader recapture is needed.
No broad suite, push or tracker changes. Temporary final inputs remain under
/tmp/shareback-repair-proof until root integration/release; reusable target retained.
