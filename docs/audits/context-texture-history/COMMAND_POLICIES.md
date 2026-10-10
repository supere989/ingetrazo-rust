# Original command ownership inventory (P15 remains open)

Pinned Python: `6be29fe437a7cd992d4e079f71720821b25216b9`. Source references are `core/commands.py`, unless stated. This inventory distinguishes ordinary command Undo from mesh-shareback wrapper Undo; it does not authorize a blanket Scene history policy.

| Commands / source line | Ordinary Undo footprint | When absorbed by ReshareInstanceCommand |
|---|---|---|
| ReshareInstanceCommand 2959 | Prototype mesh plus edited placement mesh/xform | Wrapper owns these objects only |
| SnapshotImport 2640 | Mesh plus its appended groups/layers/views/dimensions/texts | Global appended entities survive wrapper Undo |
| AssignLayerCommand 715 | Entity labels; own newly created layer | Prototype labels revert; global layer survives |
| HideCommand 772 | Face/edge/group flags, no selection restoration | Prototype flags revert; private child visibility survives |
| SetFaceColor/Opacity/MaterialTag 825/859/891 | Face attrs; own registered material where applicable | Prototype attrs revert; registry survives |
| SetMaterialFinish 935 / Restamp 961 | Registry finish / specifically captured face/group attrs | Global registry changes survive; mesh attrs restore |
| SetFaceTexture/Back 1867/1898 | Front/back face attrs | Prototype attrs revert; extracted texture files remain available |
| MakeGroup 2811 / InsertGroup 3053 | Mesh delta and own inserted placement | Wrapper does not restore entire child list |
| MakeNestedGroup 3134 / MakeComponentOf 3198 | Adoption/order and component flag | Groups-only edits have no mesh-shareback fold |
| DeleteGroup 3702 / MoveGroup 3488 | Actual owner/index / placement transform | Private child list/transform is outside wrapper mesh ownership |
| MakeUnique 2988 / GroupToComponent 3018 | Captured mesh/xform/children/axes/component or name/xform/component | Must test actual command ownership, not whole registry replacement |
| Place/Delete/Reverse/Active/Move/RotateSection 1306–1408 | Plane identity, activation, point/normal, prior state | Global section state survives wrapper Undo |
| CompoundCommand 3954 | Forward Do, reverse Undo of contained commands | Wrapper intentionally absorbs commands without invoking their Undo |

`core/scene.py` begin_group_edit 258, _leave_level 348, share_back 407 switch geometry/context while retaining global entities. Original clipboard `formats/clip.py` does not transfer the material registry despite its comment; no registry-transfer claim follows from this corpus.

## Verified bounded milestone

Foreign textured loose-face paste through actual PasteTool/context History and native application copy/preview/cancel/commit/Finish/Undo/Redo: direct and nested contexts have exact geometry, UVW constants 10/110, and referenced PNG SHA256. Previous native Finish lost all imported image bytes; retained baseline fails strict referenced-asset checking. Native implementation now publishes the context image cache at the cold Finish boundary. No desktop clipboard is changed.

## Remaining acceptance gates

Material/layer/private-group/visibility/component-registry combinations, nested adoption/move/delete and chronological history truncation need actual workflow oracles beyond this source inventory. Global guides/text/geodata are not all carried by current Rust edit contexts; investigate operative source command paths before changing them. Paint stamping may differ from source Compound color/texture/opacity/tag dispatch; operative GUI/tool audit is required.

Original saves only referenced texture files, while native preservation writes retained embedded archive assets, including unreferenced images after Undo. This corpus compares referenced assets only. Exact cache-purge/archive packaging semantics remain unproved; deleting opaque archive assets is not an acceptable shortcut. Vendor attribute null in both clipboard outputs is not evidence of arbitrary metadata retention. No rendered UI/GPU/Metal or complete P15 claim.
