# Active global image Rotate through context Finish

Actual original viewport context commands and RotateTool observe ten cases/65 exact states: plain, component, folded, nested, nested-folded, each live-Finish and cancelled-Finish. After Finish, subsequent hover/commit/Undo/Redo/Cancel match complete frame, active tool, edit state and chronological history depth. Literal previous source fails five live cases and passes five cancelled controls; three state mutations are rejected. Full workspace passes.

The narrow source fix keeps the original global image objects and lazy preview snapshot alive for image-only Rotate through Finish; it uses the existing explicit tool ownership hook. Mixed local geometry/groups/sections/copy ownership is not implemented by this hook and remains mandatory full scope. This does not claim GPU image drawing, axes/ticks/hot retyping or all Rotate workflow parity.

Replay pinned Python scripts/probe-python-image-rotate-context.py with offscreen Qt and output path; cargo run -p ingetrazo-gui --example image_rotate_context with isolated config; scripts/check-image-rotate-context.py takes original/native/literal-old JSON. Original native dimensions/style/layers state is preserved by the existing context implementation, not a new global Scene exclusion.
