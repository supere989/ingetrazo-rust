# Native Metal bounded command submission

Source96b6258, base9cf2abf. Both aarch64-apple-darwin and x86_64-apple-darwin locked example crosschecks pass; five shared gpu_memory admission/retirement tests pass on Linux. These are compile/ledger checks, **not Mac GPU execution**.

On a genuine Mac runner: `cargo run --locked -p ingetrazo-metal --example submission_probe`. The native command queue fills17MiB private GPU storage and BLITs to shared readback. Every byte is verified only after actual Completed. Actual GPU storage owns both allocations until verified completion; dropping completion-unknown work intentionally retains command/device/queue/buffer owners and budget charges for process lifetime, and poisons new admission. No unbounded waitUntilCompleted or process/backend wrapper exists. Poll deadline is at most5s.

Mandatory probe controls: malformed extent refusal before allocation; readback before completion refused; real second allocation budget refusal releases unsubmitted first allocation; zero-deadline unknown retains charges/refuses new allocation; later acknowledged completion frees safely; unknown Drop retains ledger. Zero deadline is an ownership-negative engineering control, **not a genuine stalled-driver proof**. Error completion is conservatively quarantined rather than guessed safe.

Metal Shared readback is CPU-visible only after Completed. Private GPU storage never receives CPU-written result bytes. 17MiB CPU inspection is diagnostic and does not imply renderer bandwidth or application performance.

Runner device_name must be retained verbatim and classified. Apple Paravirtual is virtual execution, not physical Mac hardware. Physical Apple/Intel GPU, real error/device-loss behavior, shaders/rendering/swapchain/texture/RT and native Metal parity remain OPEN. Unknown quarantine deliberately leaks until process teardown; recovery/reclamation is not implemented. No automatic fallback or freeing pending work.
