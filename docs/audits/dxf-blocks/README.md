# DXF block import: frozen original behavior

Thirteen original application workflows and 91 states are frozen from unchanged Python `6be29fe437a7cd992d4e079f71720821b25216b9` with ezdxf 1.4.4. Native implementation now matches these 91 states, seven supplemental workflows (49 states), and all 20 actual original-reader saved-document pairs. The original source corpus remains unchanged.

The corpus covers repeated named placements, distinct instance layers, block base points, reflection/nonuniform scale, nested block linework and curved entities, oblique OCS insertion, case-sensitive prototype ownership over case-insensitive definition lookup, mixed loose/layer/placement ordering, centimetre conversion, UTM recentring, depth limits, faces, missing/annotation-only definitions, and nested application edit/history/save.

The actual loader builds each used prototype once in local metres, then emits placements in model-space order after sorted layer groups. Nested INSERT contents flatten inside the prototype with depth eight. Inner layers collapse into the component; the placement carries its own layer. Matrix translations scale and recenter while linear terms retain their authored scale. Recentering measures ordinary points and placement translations, truncates the low bounds toward zero, and requires an X extent as well as coordinates beyond 100 km.

A documented source claim differs from actual behavior: array attributes are attached to an INSERT, whose `matrix44` path wins before exploding. `virtual_entities` also returns only the first array placement. The frozen MINSERT case therefore imports one placement, not twelve. Native parity must follow the captured original behavior and retain this limitation explicitly.

Each state records complete observed geometry, layers, units, history, curve-owner counts, local placement matrices, group/component flags and true shared-Mesh pairs. Original saves remain available for independent reopening. Source inputs were captured once; subsequent native checks must reuse the authenticated archive.

The native model currently stores a full Mesh per Group with a separate sharing identity. Avoid repeated parsing and prototype construction; do not claim constant-memory placement until native instance storage is independently addressed and measured. Source sharing and memory behavior are separate acceptance obligations.


## Native acceptance and replay

Runtime chain: `0572d9e3` → `f0efbb88`; directly affected legacy checker correction: `ed1a70d9`. Named build took 13.04 seconds, then the corrected observer/source arithmetic build took 8.72 seconds. Five focused block/curve/spline tests passed. Final binary/source hashes and every proof member are pinned in `manifest.json`; `records.zip` retains outputs, original-reader pairs, logs and both initially rejected observer candidates.

The first candidate geometry was already exact. Its observer incorrectly defaulted a classic source Group's missing `component` field to false; source construction defaults true. Correcting that observation yielded the full 140 states. The bulge radius calculation also now uses literal source `/4/b` evaluation order; this was not a demonstrated geometry mismatch in the frozen recipes. Both first observer candidates remain mandatory rejecting controls.

Authenticate `source-records.zip` with `source-manifest.json` and `addendum-source-records.zip` with `addendum-source-manifest.json`, then extract their input members by basename into one replay directory. Add the two `native/first-native-*.json` members from authenticated `records.zip`. Run the already-built `context_document_lifecycle <dir> --dxf-blocks` and `--dxf-blocks-addendum`; run `reference-dxf-blocks-reopen.py <dir>` and `--addendum` using the pinned original environment; finally run `check-dxf-blocks.py <dir>`. Required source JSON names are `original-dxf-blocks.json` and `original-dxf-blocks-addendum-all.json`. All primary/supplemental DXF, input IGZ and original saved IGZ members are required, including the primary guard inputs. No source handler recapture is needed.

Supplemental cases protect uniform rotated circles/arcs, full-ARC authored seam, INSERT-owned ATTRIB/SEQEND, anisotropic legacy/LW bulges and nested unit-scaled rational spline. Analytic geometry transforms precede adaptive flattening; sampled-point transforms are not used. Prototype parsing/building occurs once per exact authored name, with folded definition lookup. Prepared registry deltas retain nested Cancel ownership before publication. Actual MINSERT remains first-placement-only, matching the operative source.

Retained unchanged straight/curve/spline inputs were replayed on the final binary: 56/84/105 states and 8/12/15 reader pairs remained exact. The old sampled CLI checker now expects successful placed component linework rather than obsolete block refusal; that script was syntax-checked here and is left for root's integrated CLI execution.

Remaining scope: native placement Mesh payloads and cache definitions still clone full Mesh values; sharing identity does not imply constant-memory instances. Source broader face stitching/healing/fusion and unsupported DXF entities remain separate P22 work. This batch does not promote the full exchange criterion or redefine original-invalid behavior.
