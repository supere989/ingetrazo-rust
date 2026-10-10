# Integration test policy

Reuse retained results when the tested source and relevant dependencies are unchanged. Record why evidence remains applicable when integrating another branch.

Run focused checks for changed behavior and unresolved failures. Run the broad workspace suite once on the combined integration batch. Repeat GPU captures only when affected rendering code, inputs, dependencies, or unresolved findings justify them. Do not rebuild solely to reproduce an already retained result.

Retain concise results, source identity, and necessary failure evidence. Purge owned terminal temporary build, transport, and state data after evidence is retained. Never purge another active process or source worktree.

## Current batch

Base: a1f25b2bb67de5265f41c6fe8b9a6baaf209c5b6.
Walk native modifier ingress and explicit missing-snapshot refusal have been brought into this review branch. Independent review tightened complete finite three-dimensional vector checks; retained strict source/native observations pass and a truncated vector is rejected without rebuilding the application. See navigation-walk-modifier-ingress/integration-checker-review.json.

Combined build remains pending. Metal/Glow modifier ingress and real operating-system same-frame event coalescing remain open. This batch does not advance full-feature parity verification.
