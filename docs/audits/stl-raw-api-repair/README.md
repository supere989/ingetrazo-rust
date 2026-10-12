# Raw STL API repair

Published `4dc38657` Linux CI exposed three existing raw STL roundtrip failures: a triangulated cube changed from twelve facets to six coplanar faces. The GUI source cleanup policy had leaked into general import/load APIs.

The repair preserves raw facets and active-mesh append behavior in import/load APIs, retaining authored ASCII f64 coordinates. GUI parse plans retain the original Qt materialization, units, simplification and root-group policy. Strict malformed-input refusal remains; this does not claim allocation-failure atomicity.

Root focused checks pass: three legacy integration roundtrips, four related library checks, and an additional authored-precision/append regression check. The frozen GUI replay passes all eleven workflows/38 states, eight parser controls, eight original readers and seven rejecting mutations. Incremental GUI build: 9.878 seconds; full replay: 3.019 seconds; zero new Python captures. Independent runtime prereview approved the separation.

`records.zip` retains the published Linux failure, focused logs, exact tested runtime patch/hash and affected GUI evidence. The replay receipt records the base HEAD because the repair was uncommitted during execution; the archived patch identifies the tested runtime. The additional regression test changes no production runtime. Manifest hashes bind every retained file. Full platform/release acceptance remains open.
