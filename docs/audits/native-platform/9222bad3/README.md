# Published 9222 platform acceptance

Publication `9222bad37b3ae72bead49d3ed7c5379766827b9e` was observed once: full run `38186690679` and automatic ordered run `38186690614`. No manual dispatch, retry, cancellation, build or source capture was performed by the observer. Main remains `c73fe993`.

Linux passed all 113 job steps with no skips: 28 named semantic consumers and 174 successful stage records. The three DXF modes passed 35 workflows, 245 exact application states, 35 actual original-reader reopens and 97 rejected mutations. Composer pointer acceptance also passed 13 workflows, 89 states and 15 negatives. Windows, both Apple cross jobs and both ordered Metal jobs passed.

Both actual ARM and Intel Mac jobs passed all 75 formats tests, including `exact_source_curve_paths`, `exact_source_spline_paths`, oversized-degree preservation and portable bulge intermediates. This closes the new curve endpoint regression from `08726369`. Full test logs and their exact repair lines are retained in the archive.

Overall CI remains failed at the known Mac rendering gates: section pixels and accelerated CGL availability on both platforms, plus strict annotation/CLI sample-policy refusal on Intel. ARM section mismatch counts remain 0, 0, 0, 0, 0, 0, 134, 152, 134, 0. This does not establish full platform parity or physical GPU qualification.

The canonical archive contains 224 records (102,345,541 uncompressed bytes), compressed to 1,642,676 bytes. Archive SHA-256: `86abc0dd9d15fc0bb7d4d9de8ad0da8cc1129f55f55ac5fbc486a06d808b745e`. Root independently authenticated every member and checked the literal Linux and Mac results; see [root review](root-review.json). Only 716,506 bytes of the 23,291,385-byte Linux artifact were fetched to retain 18 relevant records.

CI's six source captures took 39.018 seconds and provider binding took 6.831 seconds. These timings are recorded separately and do not establish a throughput improvement. Historical replay remains disabled.

After independent archive authentication, 260 temporary duplicate files totaling 107,999,551 bytes were removed. Canonical evidence, unique failures, source, reusable worktrees, build targets, caches and the private GPU bundle were preserved. See [cleanup receipt](cleanup.json).
