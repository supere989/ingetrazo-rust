# AddEdge command ownership correction

Frozen original AddEdgeCommand owns only a newly created edge. Undo leaves both welded vertex objects in the mesh; Redo relinks the same edge. Merging an existing edge owns nothing: optional soft/curve stamps survive Undo. The operative original identity assertions and native UUID/flag tests are retained.

The app add_edge_command entry stages a mesh candidate before publication and stores the command-specific pre-edge mesh including exact orphan endpoint UUIDs. It is not a query alias or a global snapshot policy. Updated query/controller fixture invokes that production command: ten cases/27 states match the actual frozen Python handler/context/history. The literal prior snapshot implementation is rejected by the same full validator. Five existing mutated controls also reject.

Interactive drafting composites, all other command-specific selection/metadata ownership, nested shareback histories and native script mutation journal remain required scope. No claim of arbitrary Python or complete scripting atomicity. Initial fresh replay omitted original.json transport copy and failed before execution; corrected by retaining actual source stdout at that filename.
