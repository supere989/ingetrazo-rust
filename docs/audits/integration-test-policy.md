# Integration test policy

Reuse retained results when the tested source and relevant dependencies are unchanged. Record why evidence remains applicable when integrating another branch.

Run focused checks for changed behavior and unresolved failures. Run the broad workspace suite once on the combined integration batch. Repeat GPU captures only when affected rendering code, inputs, dependencies, or unresolved findings justify them. Do not rebuild solely to reproduce an already retained result.

Retain concise results, source identity, and necessary failure evidence. Purge owned terminal temporary build, transport, and state data after evidence is retained. Never purge another active process or source worktree.

## Current batch

Base: public main `039a5b63d1160011faaa196bfb3cd69af4e5645c`.
The review branch now contains nested/tagged/shared SKP placements, embedded default texture import with aggregate asset admission, and pure MCP protocol/error guards. Root has independently checked all 23 complete retained SKP document comparisons and their compressed/uncompressed hashes. The merged MCP protocol already passed its three focused tests.

Pending intake: source sheet-border rendering, exact default-layer behavior including paper visibility, and the live MCP application queue. Keep these coherent workstreams frozen before starting the one combined workspace run. Relevant focused source/reference results remain attached to their commits; do not repeat them merely to reproduce an unchanged result.

Metal raw texture/host/font work remains in `review/metal-raw-canvas` and has separate actual macOS checks. Its successful raw-pixel fixture and host compilation are not Canvas text acceptance: the current Linux-only font provider is an explicit open dependency. Native CoreText matching and actual host rendered comparison remain required. Do not trigger broad CI for evidence-only updates to that isolated branch.

After combined validation, retain the tested source revision, result log and scope, then purge only the owned terminal target. No full feature or release gate is promoted by compilation or a bounded fixture alone.
