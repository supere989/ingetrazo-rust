# Nonblocking native Metal CAD Drop audit

Before deciding that frames are unknown, Drop now calls the existing nonblocking
actual command-status collection. Completed commands release their owned resources
without requiring the application to call poll_completed explicitly. Pending or
Error status still retains the entire owner and poisons admission. No wait/sleep
was added to production Drop. Prior explicit timeout unknown flags remain
conservatively retained; this change does not infer recovery from elapsed time.

Two actual-native GPU tests in cad_drop_controls are cross-compiled, NOT yet
executed here. Run `cargo test --locked -p ingetrazo-metal cad_drop_controls --
--nocapture` on macOS. The positive submits ordered-span compute, independently
observes actual Completed without collecting the renderer, then Drop must free
all charged data. The negative submits behind a real initially-unsignaled Metal
shared-event GPU wait: Drop must poison/retain the entire ledger. Signaling the
event and later actual completion does not retroactively free quarantined data.
Test waits are explicit diagnostics <=5s, never invoked by production Drop.
Native AppleParavirtual execution is distinct from physical Mac validation.
