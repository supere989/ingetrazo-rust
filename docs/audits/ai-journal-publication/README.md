# Checked append publication followup

Prepared MeshVertex/MeshEdge records publish the planned real array IDs directly, with checked extents and reciprocal adjacency. Publication never consults add_vertex or its private weld index. Four focused tests pass: discard, stale version, stale private spatial index plus duplicate public keys, and malformed edge endpoint refusal before semantic mutation. The retained eight-program source corpus remains exact; five mutated candidates remain rejected.

This is standalone and append-only. Source-compatible numeric welding, full ownership/history/deadline coverage and allocator-failure atomicity remain open. try_reserve protects live vectors and adjacency before semantic publication, but temporary preparation and spatial-index rebuilding still use ordinary Rust allocation. No OOM atomicity claim is made.
