# Mixed Move array retyping boundary

Actual original Move controller/History and native production IngeTrazoApp.dispatch_tool_vcb/Undo/Redo execute mixed selection: painted4x4face with2x2hole/vendor71/affineUVW, two soft edges sharing sourcecurve91, and an ordinary translated group. First18mcopy then3x → /3 →2x. Source uses actual Qt QMainWindow/Viewport and GL preview, not mocked status/preview callbacks. Native private locked executable provenance retained.

First3x matches literally including40edge coordinates. Faces/holes/vendor/translated UVW, freshcurvepair grouping, sourcecurve91presence, groupplacements and one history command remain equal for all subsequent states and Undo/Redo. **Full original geometry parity fails**: originaledge counts40→56→54, Undo50/Redo54; native40→40→30, Undo10/Redo30. Every edge endpoint is retained, not counts alone. Default checker exits1; --retain-open requires only those literal edge/endpoint differences and leaves passed:false.

Source basis: tools/move.py430-447 retyping calls History.undo before newbuild; AddFaceCommand.undo core/history.py629-642 calls mesh.remove_face; core/mesh.py885-898 expressly retains boundary edges. Old copied face/hole wires remain after Undo, then retyping adds newgeometry. Native snapshot restoration removes the full copy, preserving10original edges on Undo and restoring30finaledges on Redo. This is a document-integrity correction boundary, not literal parity completion.

Arrays with arbitrary crossing/overlap, spacing units during drag, pointer live preview, Ctrlgesture, context/componentarray edits and performance remain separate. No blanket Verified claim from facecounts or oneUndo-stack length.

## Complete native integrity control

The fresh native production GUI controller fixture additionally asserts exact baseline Mesh, Groups and Selection equality after Undo; exact final Mesh and Groups after Redo. Mesh equality includes opaque attrs/UVW/vertexIDs/edge flags and every topology entity. A subsequent stale4x array request refuses and leaves the entire Scene unchanged. These are source-user-integrity corrections documented separately from the retained literal source ghost-wire failure, not copied face-only counts. Native-integrity.json freezes the freshly executed executable hash.
