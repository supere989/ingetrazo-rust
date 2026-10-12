# DXF block import: frozen original behavior

Thirteen original application workflows and 91 states are frozen from unchanged Python `6be29fe437a7cd992d4e079f71720821b25216b9` with ezdxf 1.4.4. This is source evidence; native implementation and acceptance are pending.

The corpus covers repeated named placements, distinct instance layers, block base points, reflection/nonuniform scale, nested block linework and curved entities, oblique OCS insertion, case-sensitive prototype ownership over case-insensitive definition lookup, mixed loose/layer/placement ordering, centimetre conversion, UTM recentring, depth limits, faces, missing/annotation-only definitions, and nested application edit/history/save.

The actual loader builds each used prototype once in local metres, then emits placements in model-space order after sorted layer groups. Nested INSERT contents flatten inside the prototype with depth eight. Inner layers collapse into the component; the placement carries its own layer. Matrix translations scale and recenter while linear terms retain their authored scale. Recentering measures ordinary points and placement translations, truncates the low bounds toward zero, and requires an X extent as well as coordinates beyond 100 km.

A documented source claim differs from actual behavior: array attributes are attached to an INSERT, whose `matrix44` path wins before exploding. `virtual_entities` also returns only the first array placement. The frozen MINSERT case therefore imports one placement, not twelve. Native parity must follow the captured original behavior and retain this limitation explicitly.

Each state records complete observed geometry, layers, units, history, curve-owner counts, local placement matrices, group/component flags and true shared-Mesh pairs. Original saves remain available for independent reopening. Source inputs were captured once; subsequent native checks must reuse the authenticated archive.

The native model currently stores a full Mesh per Group with a separate sharing identity. Avoid repeated parsing and prototype construction; do not claim constant-memory placement until native instance storage is independently addressed and measured. Source sharing and memory behavior are separate acceptance obligations.
