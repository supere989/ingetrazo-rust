# Shared-budget outline admission, bounded proof

Frozen production/source revision a19d38a0. Real NVIDIA RTX 5090 owned-window Vulkan proof passed; no software substitution. Source shader/draw ordering is unchanged, so the retained unchanged-source 29-frame evidence is reused rather than repeated.

- A real 40 MiB page passes the former 32 MiB per-pool ceiling.
- A second 48 MiB request under a real shared-ledger cap refuses while the first page has a Pending range. No pending page is reclaimed.
- After rollback makes the 40 MiB page completely free, pressure retires and physically collects it using the actual completed graphics fence; the 48 MiB replacement succeeds.
- A separately sampled actual 48-byte VkMemoryRequirements quota refuses the preferred one MiB page, then admits exact 48 bytes. No fake Vulkan error or driver OOM was injected.
- Probe charge returns to its baseline. The owned baseline and post-probe whole frame match exactly, and more than twenty subsequent warm frames add no uploads, submissions, mappings or buffer allocations. OS close returns committed_bytes=0.

Five focused planner tests pass (including 96 MiB page/40 MiB reservation, Pending protection, fragmentation, and retired serial exclusion). The focused GUI admission classifier test passes. Final native fixture cargo check/build passed. An initial unsupported feature flag failed before compilation; corrected command uses the platform default Vulkan code. An actual first disposable transport failed before Vulkan because XKB data was missing; its logs are retained. Read-only XKB_CONFIG_ROOT corrected transport without changing source.

Scope: this is real page allocation/admission/reclamation, not a 48 MiB geometry rendering throughput benchmark. Unknown-completion range ownership retains the prior planner and Resources queued-fence evidence; no new real queued-arena control or device-loss claim. Logical reclaimed_bytes differs from actual allocation ledger accounting in general. Page enumeration occurs only after explicit shared-budget pressure, not unchanged warm frames. Empty pages otherwise remain reusable. Native Metal analogous range consumption and full fragmentation/streaming performance remain open.

Transport: disposable network-none dev-base, NVIDIA CDI, read-only published container runtime/venv, read-only exact Xwayland socket/auth, VK_DRIVER_FILES GLX NVIDIA ICD, XKB_CONFIG_ROOT=/runtime/usr/share/X11/xkb, LD_LIBRARY_PATH=/runtime/usr/lib/x86_64-linux-gnu:/runtime/lib/x86_64-linux-gnu:/usr/lib. Only the newly-created exact fixture client was captured; no desktop capture.

Named fixture SHA256: `15b86e5ec0b12ea2800cddf16538727e6a31ef0a86fad7b96dd6e05f2f7e6a3d`.
