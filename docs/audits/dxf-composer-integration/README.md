# DXF and composer integration acceptance

Runtime `147b9a9104e80aa5125dcd1a4976151292ff9caf` passes one combined affected build in **10.931s**, followed by all three semantic lanes in **3.034s**. No original recapture, broad local suite or rendered check repetition. Existing dependency artifacts were retained; changed local workspace packages were invalidated on target handoff.

- Legacy DXF POLYLINE: 8 cases, 56 exact App/history states, 8 original-reader reopens and 16 rejecting mutations.
- DXF circles, arcs, ellipses and bulges: 12 cases, 84 exact App/history states, 12 original-reader reopens and 36 rejecting mutations. Source cubic construction and OCS-before-flatten geometry is shared, with source curve ownership preserved.
- Composer item dragging: 13 actual source recipes, 89 exact states and 15 rejecting mutations. Press acquisition, cached snap targets, hidden/locked stacking, source Escape behavior, popup ownership, lost release and independent history for two gestures in one input packet pass. The owner also ran all 34 affected composer tests once on the final correction.

Independent receipts accept both DXF subsets, the corrected composer runtime and shared CI wiring. CI builds 30 examples once for 28 semantic consumers plus the two retained rendered consumers. The new DXF modes reuse the existing context binary. Source archives are authenticated, and every new runtime/checker carries its producer dependencies.

`receipt.json` records exact commands, times, binary hashes, source archive hashes and checker bindings. `records.zip` was reopened and compared byte-for-byte with all retained raw files before publication. The runner, source inputs, outputs and actual original-reader observations are included. Unique owner failures remain in their component archives.

P04 remains Partial; P22 remains Implemented. SPLINE, block instances and broader import fusion remain separate work. Composer resizing, multiple-item gestures and broader mutation coverage are outside this finite pointer corpus. All five release gates remain open. The prior published `df2a4830` passed all Linux steps, Windows, Apple cross jobs and ordered Metal; its known native Mac gates remain open. This batch awaits one publication CI run.
