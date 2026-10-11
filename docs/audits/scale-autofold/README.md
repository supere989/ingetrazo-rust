# Scale autofold core boundary

Source core/topology.py fold_nonplanar_faces at1440: test all outer/hole vertices with1e-3 Qt plane tolerance, triangulate through source earcut, merge only pieces of the same mother at normal dot>.999. Core document coordinates remain f64. New pieces carry mother attrs/named layer and fresh face identity; untouched records/vertex UIDs and source boundary edge tokens persist. Atomic finite-input refusal keeps original model.

Three exact oriented-ring/holes/attrs/interior controls: lifted quad, gentle collinear hex, unchanged planar hole. Warped hole has original Python set-object-identity merge ordering: two retained source runs yield different planar partitions. Native deterministic partition matches exact lifted triangles, face count, metadata and projected hole coverage; literal planar partition parity is unresolved/nondeterministic and is not labeled exact. Six corruptions reject. Two focused core tests pass. Initial test failure compared incidence lists despite newly added fold edge; corrected assertion compares actual preserved UID/position and all existing edge tokens, retains derived incidence updates.

API autofold::needs_fold(&Mesh)->bool; fold_nonplanar_faces(&mut Mesh)->Result<FoldReport,String>. Report exposes folded/created runtime history IDs. Cold candidate clone only when warped face exists; no ScaleTool edits or whole-workspace run. Native app wiring is parent's separate Scale lane, not proved by this core fixture.

Replay named core example scale_autofold SOURCE OUT, then parity Python scripts/check-scale-autofold.py SOURCE OUT COMPARISON. Source reference scripts/reference-scale-autofold.py OUT.
