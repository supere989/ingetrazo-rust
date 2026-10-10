# Genuine VM Group and Mesh ownership prerequisite

Status: design only; production scripting remains disabled. Source pin: `6be29fe437a7cd992d4e079f71720821b25216b9`.

## Operative source contract

`core/ai.py:68-83` assigns `tuple(color)` separately for each face before orientation and softening. Tuple input may retain identity; list input creates distinct tuples; an iterator is consumed independently per face. A truthy arbitrary `name` is retained by `Group.__init__`, while falsy names consume the module-global automatic-name counter (`core/group.py:20,50-52`). These operations can execute Python callbacks and fail before scene publication.

`extrude` iterates `outline` twice (`core/ai.py:126-127`), creates side faces in outline order, then reversed lower cap and upper cap. Materializing a one-shot iterable once and reusing it would change source behavior. Empty and degenerate outlines remain required controls.

`orient_outward` clears interior flags and preserves winding for a coplanar mesh (`core/orient.py:406-412`). The existing Rust `heal::orient_faces` additionally aligns planar winding, so calling it unchanged is not source-compatible. Source normal arithmetic combines double Newell accumulation with Qt float32 construction, length and division; softening uses the Qt float32 dot and strict greater-than cosine(42 degrees).

## Owned publication design

A genuine VM Mesh payload owns the native mesh journal and actual Face references. A genuine Group payload retains that exact Mesh reference and actual Python name/metadata objects. Scene membership commands retain the same Group reference across Undo/Redo; detached scope references continue to observe its Mesh and Face objects while membership is undone. No JSON conversion or coordinate-based reattachment is allowed.

New helper geometry is privately staged. Geometry, radial allocation, handle vectors, history capacity and logical leases must be prepared before scene publication. Color conversion, name truth testing, orientation and softening run before publication; any caught host admission refusal aborts the stage. Existing-object mutations require a separate mutation journal and are not made atomic by this new-object publication design.

The scene command must distinguish loose-mesh changes from group membership changes so a group-only command does not fabricate a loose-face history entry. Group and Face handle vectors move with their leases rather than being cloned. Underlying native mesh storage needs an owned extraction boundary; it must not clone the freshly built mesh merely to wrap it in a Group.

## Focused acceptance sequence

1. Actual original Group identity corpus: returned Group, same Mesh, same held Face, Undo removal, Redo restoration and alias mutation while undone.
2. Real typed Mesh construction followed by Group membership publication; branch truncation releases membership leases while scope-held objects remain alive.
3. Extrude lists versus one-shot iterables, per-face list/tuple/iterator color identity, arbitrary truthy names and default-name counter.
4. Source planar/degenerated/nonplanar orientation and strict softening controls before helper parity claims.
5. Caught admission failure, failed history preparation with preexisting Redo, stale owner tokens, fuel poison and unwind discard.

Full Vertex/Edge mutation, mutable existing attributes, Group children/placements, all source helper functions, arbitrary imports and comprehensive VM/native memory or wall-time bounds remain required. This design does not authorize production activation.
