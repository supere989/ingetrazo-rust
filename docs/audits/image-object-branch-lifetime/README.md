# Detached image metadata branch lifetime

Actual source History Add/Undo holds the original object while detached in Redo, then releases it when a new command truncates that branch. Exact original class is GC-tracked, its identity retained without a strong reference, and replacement identity asserted distinct. Native command-identity metadata follows same three states. Literal0aaf old implementation retains dead metadata and is rejected.

Cold collector runs only on redo branch truncation or history-limit eviction; roots live/current and history snapshots, explicit delta-held image identities, parked/cancel contexts and their histories. No coordinate matching. Source archive/runtime image geometry unchanged. Two meaningful regression tests also preserve cancelled-context redo identities and exclude runtime cache from serialization. Workspace552passed/2ignored, prior56 and layer19 protocols pass.

Remaining: snapshot payload duplication and finer command-only roots, cache compaction on every context-history maintenance path, all runtime/global allocation budgets. This bounded branch leak correction is not full memory optimization. Graphics/tool and transform command parity remain open.
