# Finite P05 source reader and triangulation oracle

This captures only the missing source behavior: nine actual `save_scene` → `load_into` outcomes and sixteen actual `Face.triangulate`/`core.triangulate` inputs. Original source is pinned at `6be29fe437a7cd992d4e079f71720821b25216b9`. Production Rust behavior has not been compared by this source-only handoff; P05 remains incomplete.

The nine inputs and all 27 retained construction/Undo/Redo states remain unchanged in `docs/audits/ai-degenerate-faces/original.json`. Before saving, each captured topology was asserted exactly against its existing construction record. No history session was recaptured. All nine source files save and load successfully using the actual **plain** reader (estimated corner count below 1024). This does not establish bulk-reader behavior. Eight loaded topologies equal construction; `welded-close` has different reader vertex encounter ordering, retained literally. Native load must match the source loaded topology rather than assume construction identity.

The triangulation inventory contains the nine degenerate inputs plus collinear outer, L-shaped concave XY/reversed/YZ/oblique, one proper hole, and two equal-height holes. Full ordered triangle coordinates are retained separately for the actual welded Face entry point and the direct raw-ring triangulator. `welded-close` differs between these entry points; it must not be normalized away. Counts, zero-area and area diagnostics are informational only. No sorted triangles, geometric tolerance, or area-only acceptance substitutes for literal ordered output.

`records.zip` contains `original.json` and nine actual original IGZ files. `manifest.json` authenticates every member. The reference records source helper hashes, script hash, input hash and unchanged history hash. Source capture used `/tmp/ingetrazo-parity-python/bin/python`, offscreen Qt, and completed once with nine successful loads and sixteen triangulation inputs. Owned raw records remain at `/tmp/p05-finite-kernel-proof` pending parent review/release.

Commands inside `ingetrazo-publish`:

```sh
/tmp/ingetrazo-parity-python/bin/python scripts/reference-p05-finite-kernel.py /tmp/p05-finite-kernel-proof
python3 scripts/check-p05-finite-kernel.py
python3 scripts/check-p05-finite-kernel.py /path/to/native.json --report /path/to/comparison.json
```

The first checker command authenticates the canonical source inventory and nine rejecting mutations; it does **not** claim native acceptance. The native protocol uses schema 1 and the same `save_load`/`triangulation` records. Native provenance/archive bytes/error prose/area diagnostics are not geometry equality fields; literal outcome booleans, input rings, normal, loaded/constructed topology and complete ordered triangle coordinates are. Native refusal against a source success is a failure. Exceptions remain explicit outcomes. `p05_complete` stays false.
