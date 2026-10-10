# Owned Xvfb cleanup correction

Run 38052021093, job114212896917, source8c40a462:
the native startup/resize/application-close gate passed, then Xvfb termination timed out after five seconds. The suite therefore stopped after its first case; ten-case rendering parity was not established by this run.

The cleanup helper acts only on the Popen handle created by this checker. It preserves the five-second terminate deadline, records the timeout, sends SIGKILL only to that still-live owned process, and requires bounded reaping. Application close, suite deadline, stencil comparisons and all acceptance thresholds are unchanged. Rendering failure remains a failure.

Fresh real process controls cover ordinary termination, SIGTERM refusal with bounded kill/reap, already-exited process, and a separate live sentinel that must survive cleanup.
