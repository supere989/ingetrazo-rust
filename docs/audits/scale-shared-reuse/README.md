# Shared Scale arithmetic reuse

Shared owners with identical ordered coordinate bits reuse one prepared coordinate payload. Hash collisions are resolved by full bit comparison; signed zero and one-bit differences stay separate. UV coefficients are grouped independently, while each owner retains its own vertex identities and texture metadata. Every selected-owner command still executes its original sequential Qt-f32 arithmetic, avoiding matrix-power rounding changes. Warm motion reuses captured scratch; history replay keeps its cold path.

Focused identity/metadata/rounding and warm-storage checks passed (2 tests). Frozen original inputs produced 74 exact base states plus the documented mirror-registry integrity difference, and 30 exact shared phases (10 overlap the base corpus). Geometry independently reviewed the source without another build. No original recapture or broad suite was needed in this owner tree.

A bounded A/B dev-profile observation used 65 owners, 64 selected commands, 256 vertices and 225 textured faces per payload, and four commits including finite admission. Both fresh binaries produced the same state digest. Commit time was 5.9331 s before and 0.1433 s after (41.4x observed). Capture rose from 19.4 ms to 29.5 ms. This is one synthetic shared-content observation with debug=0, not a release/application performance claim. Original per-owner baseline copies remain; mapped scratch adds memory at capture.

`records.zip` retains logs, native states, source inputs, executable hashes and source hashes; `manifest.json` authenticates every member. `comparison.json` provides the two measurements and blob identities. The example `scale_shared_reuse` is the reproducible fixture. Source original remains pinned at 6be29fe437a7cd992d4e079f71720821b25216b9.
