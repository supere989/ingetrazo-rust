# Native Python scripting and document transaction design

Status: feasibility design, not an implemented interpreter or atomic scripting claim.

## Frozen operative contract

`core/ai.py` at6be29fe build_scope publishes viewport, scene/model, mesh, selection, groups, layers, bim and native-like QVector3D/Mesh/Group/Vertex/Edge/Face classes. Recipe functions include revolve/extrude/prism/wall/house. Each request rebinds live root objects in a persistent scope; scripts can retain other references and import arbitrary modules. run_transactional first compiles eval, otherwise exec; non-None expression results update `_` and print repr. stdout/stderr/traceback are captured. One SnapshotImport history step is removed for a detected no-op, restoring the prior redo branch. Existing source rollback does not cover all document mutations: retained transaction-scope controls prove this, so that source defect is not an acceptable native atomicity implementation.

## Genuine Rust interpreter candidate

[RustPython](https://github.com/RustPython/RustPython) implements Python in Rust. [Embedding API](https://docs.rs/rustpython/0.6.0/) and [native-module interface](https://rustpython.github.io/docs/interop/rust_from_python.html) allow owned native classes/modules. Registry metadata checked2026-10-10: rustpython/rustpython-vm0.6.0, MIT, MSRV1.95; isolated compiler1.96. This is an embeddable language VM, not a recipe text recognizer or CPython/PySide process wrapper. Production dependencies/features and runtime performance have not been accepted. Disable unneeded default host/network/SSL/allocator/JIT features in the first bounded feasibility executable; include actual compiler and only required standard modules. Later full import compatibility must be explicitly inventoried rather than claiming all CPython C-extension modules work.

First feasibility gate: persistent native VM with functions/classes/loops/import math, eval-vs-exec behavior, `_`, stdout/stderr and genuine tracebacks; native QVector3D and staged Mesh class operations; SyntaxError/runtime-error/no-op controls. Literal scripts must run through the interpreter, not translated by regex. No external provider request or user document mutation is needed.

## Transaction ownership and efficient publication

Expose staged typed proxies, not unrestricted mutable Scene references. Immutable base geometry/resource snapshots use stable IDs and shared binary storage; first writes create command-owned deltas (vertices/edges/faces, placement poses/membership, materials/layers/annotations/presentation and asset registry). Multiple edge calls share one transaction journal; do not call the per-command Scene snapshot helper in each loop. All exposed mutators need before-images and validation before UI-thread publication. A refused/failed script discards unpublished deltas and adds no history; a successful changed script publishes one validated command, with original no-op redo preservation. The current AddEdge command API preserves its specific endpoint lifetime but still snapshots one Scene per command; it is not the final script hot path.

Persistent scope references need explicit live/staged/detached object ownership. Undo/reload/context exit cannot retarget a stale handle just because coordinates coincide. Proxy arenas resolve actual object identity and handle generations, preserving source detached-object semantics where required. Journal branch truncation/reclamation must retain referenced objects only as long as source command/scope ownership requires.

UI-thread dispatch remains the only live-document publisher. Long/unbounded scripts cannot be advertised as cancellable solely by checking a queue deadline. Investigate VM instruction/deadline checks and a persistent Rust-owned interpreter worker with shared immutable inputs if measured/UI responsiveness requires isolation; never invoke a Python wrapper or start a process per request. A deadline before publication discards the journal; external file/network effects are outside a document rollback promise and need their own explicit contract. No GPU command crosses a worker boundary.

## Open acceptance gates

All original imports/classes/recipes and direct attrs/list mutation need adapters, including richer Qt numeric and matrix behavior, history/context/shareback lifetimes, assets and annotations. Genuine arbitrary source script scope remains OPEN until that compatibility is proved. Complete journal coverage, no-op/redo/partial-error controls, branch/handle reclamation and bounded execution remain OPEN. Screenshot/render capture and native MCP launcher/provider live transport are separate full-scope obligations. Current explicit unsupported responses are staging and cannot close P32.
