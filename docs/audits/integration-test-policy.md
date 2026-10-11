# Integration test policy

Reuse retained results when the tested source and relevant dependencies are unchanged. Record why evidence remains applicable when integrating another branch.

Run focused checks for changed behavior and unresolved failures. Run the broad workspace suite once on the combined integration batch. Repeat GPU captures only when affected rendering code, inputs, dependencies, or unresolved findings justify them. Do not rebuild solely to reproduce an already retained result.

Retain concise results, source identity, and necessary failure evidence. Purge owned terminal temporary build, transport, and state data after evidence is retained. Never purge another active process or source worktree.

## Current cadence

The latest published integration is `55d76ef2`: 888 local workspace tests, doctests and example checks passed, followed by Linux CI and six Apple checks. See [the retained checkpoint](selection-inward-integration/README.md). Reversed Push/Pull model changes have a later focused integrated proof. The accepted local `804cd732` batch adds Line ordering and shared sweep packets with 900 passing tests and four jointly built consumers; actual Vulkan/Metal rendering remains in independent workstreams.

- Reuse each owner's existing checkout and target. Build all affected named consumers in one Cargo invocation. Do not run broad suites in owner worktrees.
- Review shared interfaces early so Vulkan and Metal work can proceed concurrently. Wait for a coherent implementation before the combined integration suite.
- Run only the failed or unfinished stage after an environment correction. Do not recapture unchanged original inputs.
- Keep local implementation moving while hosted platform jobs run. One owner monitors each platform run and retains its terminal result; other agents reuse that receipt.
- Keep allocation-only Metal changes on the isolated branch and run the affected platform. Restore both Apple runners for a Metal rendering change. Do not trigger broad CI for evidence-only updates.
- Archive and byte-verify terminal temporary outputs before purging them. Preserve active targets and canonical original captures. Rotate targets only for a concrete disk or validity reason.

No feature or release gate is promoted by compilation or a bounded fixture alone. Current feature scope and remaining acceptance belong in [the parity tracker](../PARITY_PROGRESS.md), so this policy does not maintain a second feature backlog.
