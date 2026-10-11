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

Shareback uses the Qt mapping law only when both the input positions and frame
roundtrip f32 exactly. Otherwise native shareback uses f64 mapping/inverse,
including identity and integer translation of a referenced 1+2^-40 coordinate.
Change Axes retains its explicit Qt policy. This is
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

Review followup: native mapped outer/hole rings require at least three distinct
IDs, with no repeated IDs or duplicate hole memberships. Invalid rings refuse
before publication; no full topology classifier is introduced. Numeric UVW
coefficients and computed outputs must remain finite before JSON conversion;
a finite 1e308 coefficient under scale(0.5,1,1) now refuses rather than becoming
JSON null. Four malformed-ring controls, identity/translation precision controls,
and the overflow refusal preserve the original input records unchanged.
