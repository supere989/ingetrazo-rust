# Remaining native cost and allocator admission before scripting production

This is a read-only source audit of pinned rustpython-vm 0.6.0, whose archive/commit/license are recorded in tools/ai-interpreter-proof/vendor/PROVENANCE.md. No costly operation was executed. The VM fuel patch counts bytecode dispatch and explicitly does not bound wall time or native allocations.

| Actual source entry | Existing behavior | Required additional contract |
| --- | --- | --- |
| vendor/rustpython-vm/src/vm/compile.rs:495, compile_with_opts | Tokenizer/string warning passes then compiler call; no host budget poll in compiler dependency | Audit parser/codegen/native warning callbacks; source-length limits are admission only, not a proven deadline. Poll or bounded implementation requires a separate minimal dependency patch review. |
| src/builtins/int.rs:148, inner_pow | Estimates result bits and, for large results, probes try_reserve; then native Pow::pow | The probe is temporary allocation, not shared retained-memory accounting; native compute cannot be interrupted by dispatch fuel. Result/scratch bit admission and audited bounded/chunked computation must precede production. Modular power/shift/conversion also need inventory. |
| src/sequence.rs:99, mul | Overflow and MAX_MEMORY_SIZE checks, fallible result reserve, then native clone/extend loop | Existing checks are useful but not aggregate session accounting or deadline polling. Add shared allocation admission and bounded polling for repeats/concatenation; all clones must be audited. |
| src/stdlib/time.rs:128, sleep | Native nanosleep for remaining requested duration, with signal handling on EINTR | Dispatch fuel cannot stop the sleep. A host monotonic deadline must reach native blocking waits; bounded wait slices preserve requested behavior until host cancellation. Do not relabel fuel as time. |
| Error formatting/finalization | Python-defined string/repr/finalizer work may execute while formatting or dropping retained exception state | Retain raw exceptions; define guarded formatting/finalization ownership and deadline policy. Journal discard alone does not prove later VM work bounded. |

The private controller currently guards document append publication, not all persistent Python globals or all native work. Its RAII guard discards staging on Rust unwind and permanently refuses that interrupted controller. It does not promise safe reuse or bounded VM destruction. Host-controlled reset is reached only after successful command consumption or transaction discard.

The retained-history payload cap accounts Delta records and actual record/adjacency capacities, but not history stack capacity slack or VM/cold preparation memory. Append staging entry caps poison the transaction on admission failure, including a caught Python exception. Cold lookup preparation reserves major containers fallibly, yet BigInt keys and VM object/compiler allocations remain outside comprehensive fallible admission. Owner constructor/replace/Undo/Redo still contain infallible pre-mutation collection allocations. The next allocator step is fallible preparation of those containers plus explicit shared budget ownership; it must preserve old tokens, history and document on refusal. It must not silently truncate valid geometry or silently evict command history.

Production run_python remains disabled. No compiler/native fork beyond the reviewed dispatch hook is introduced by this document. Full original imports, scene operations, script values/stdout/tracebacks, screenshot execution, command-specific object lifetime and all-document rollback remain required. No CPython or subprocess compatibility wrapper is proposed.

## Actual VM face-handle integration requirements

The current review-stage `vm_faces` adapter uses genuine RustPython objects rather than serializing attributes into JSON. Before integrating its handle journal, enforce the following alongside the existing geometry transaction contract:

- A host handle reservation or admission failure must poison the whole staged transaction even when Python catches the exception. Earlier staged faces must not publish; old mesh, Redo and unrelated earlier transactions must remain usable.
- Transfer the accounting lease for moved handle storage from staging to history. Account retained capacities and separate parked/live reservations explicitly; do not double-charge the same moved allocation or present logical pointer accounting as a bound on VM object payloads.
- Held references to aborted faces remain detached. Later faces must never acquire those references merely by reusing an ordinal. Undo/Redo must retain actual object identity and shared mutable aliases, including mutation while the object is parked.
- Geometry and handle publication must perform all fallible preparation before either becomes visible. Allocation refusal, branch replacement and teardown must preserve consistent ownership and release the corresponding leases.

These are review requirements, not completed acceptance. The fixed triangle callback is a prerequisite, not the complete original Mesh API. Arbitrary mutations of preexisting aliased objects, Group/_finish identity, native metadata projection, compiler/native allocation bounds and production scene integration remain required.
