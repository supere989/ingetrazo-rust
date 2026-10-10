# Isolated RustPython VM budget prototype

Pinned crates.io rustpython-vm0.6.0 archive; package SHA and upstream commit recorded in upstream-package.json. MIT LICENSE copied from pinned upstream root, since the crate archive does not contain it. Original .cargo_vcs_info.json retained. Only tools/ai-interpreter-proof has a local crates.io patch; production workspace dependencies are unchanged.

Five modified upstream files, auditable docs/audits/ai-vm-budget/upstream.patch: private VM fuel/poison and host APIs; main/thread initializer fields; opcode tick outside Python exception dispatch and generator-collapse guard; compile refusal for threading/JIT; omit four native module registrations exposing fresh interpreter/native bypass scope. The frozen synchronous dummy _thread remains and its callback is tested under the same budget. Full import compatibility is OPEN.

Compilation/native builtin work/allocation are not wall-time bounded. Host reset has an explicit journal-discard obligation; production controller coupling not established by this standalone proof.

Additional isolated editable-discard prerequisite: vm/mod.rs adds host_budget_abort, which poisons execution before arbitrary staged Python payload release without reset. The original five-file patch remains retained unchanged; the incremental two-line hook is retained in docs/audits/ai-editable-discard-boundary/abort-hook.patch. Production dependencies remain unchanged.
