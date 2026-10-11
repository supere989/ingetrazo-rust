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

## Separate finite coverage checker

`check-p05-finite-kernel.py` remains unchanged and strict. The independent `check-p05-finite-invariants.py` reports its strict result and retains complete ordered source/native triangle differences and the Qt-f32/native-f64 normal difference. It does not call differing diagonals or normals exact and makes no pixel-parity claim.

The nine degenerate/short-ring cases require exact ordered triangles and normals. Other finite cases use rational arithmetic over the actual binary floating-point coordinates: authored-corner membership, planarity, authored winding, source triangle count, exact projected area, boundary/internal segment multiplicity, edge crossing, triangle-interior overlap, and centroid solid-domain membership. Equal-height source holes contain long collinear bridge edges; **coverage counters only** subdivide at authored vertices, without rewriting any raw triangle. Native nondegenerate normals must be finite, nonzero and exactly parallel to the authored Newell direction. Their length must either match the literal source Qt normal or be unit within 1e-12 f64 roundoff; this does not relax geometry comparisons. Eight changed-geometry/input/normal negatives reject, including twice-scaled and opposite normals.

The reopen helper defaults to the repository original-source submodule/symlink, so the same reader runs locally and on CI without a /work path assumption.

Native acceptance additionally requires exact 27 App history states, nine native-reader source-file loads, and nine actual pinned Python-reader reopens of native-written files. The initial failing candidate must be supplied with its SHA256 and must fail both strict comparison and the changed short-ring invariant check. Its canonical archive belongs to the parent acceptance audit, not this source-only archive.

```sh
/tmp/ingetrazo-parity-python/bin/python scripts/reopen-p05-finite-kernel.py /path/to/native-output-directory
python3 scripts/check-p05-finite-invariants.py /path/to/native-output-directory \
  --initial /path/to/authenticated-initial-native.json \
  --initial-sha256 519bc52b68587c56ed3c4f5683d7945ad9de24bd0e862c808b716b2eb9237aa9 \
  --report /path/to/invariants.json
```

The independently inspected corrected consumer at `/tmp/ingetrazo-text-finite-integration/mesh` passes all sixteen finite cases, 27 history states, nine source-file native loads and nine actual Python reopens. Its strict result remains **false**: one oblique normal uses native f64 rather than Qt f32, and proper-hole/equal-height-hole ordered diagonals differ. Parent retains the fresh native files/report and initial failure under `docs/audits/p05-finite-acceptance`; these are not a source recapture or a new Rust build by this observer.

The retained `invariant-source-check.json` is the earlier source-only self-check (six geometry/input negatives). The unit-admission followup runs only the changed final native checker and adds two normal negatives; its fresh output is parent-owned `invariants.json`. The original source/IGZ archive is unchanged.
