# Exact pixel comparison fast path

Five PDF page checkers now test whole-buffer byte equality before counting individual changed pixels. Fully identical images return the same zero difference; differing images retain the original detailed counting expression. Dimensions, text, nonblank source checks, case rosters and negative controls are unchanged. No tolerance was added.

A single paired replay of the retained vector corpus at 300 DPI produced the identical 26-page JSON report and all 52 PNG files were byte-equal to the prior archive. Measured elapsed time was 24.40 seconds before and 8.34 seconds after, including Qt PDF load/render and PNG writes. This local sequential observation is cache/load sensitive; it is not an overall CI speedup claim. The live prior CI spent 50–53 seconds in each complete vector mode, motivating this change.

[Acceptance](acceptance.json) and `measurement.zip` retain the old reader, paired results/times, exact source-archive hash and the initial unavailable timing-tool setup failure. The source/native PDFs were reused; no Cargo call or original application capture occurred. Temporary duplicate PDFs/PNGs and measurement state were removed only after archive byte verification; canonical inputs and build caches remain.

## Observed Linux CI page checks

The six unchanged strict PDF page gates all passed on published `55d76ef2`. Their recorded step time totals 143 seconds versus 308 seconds at `9cd824db` (165 seconds less across these six steps). Each current step took 23–24 seconds versus 50–53 seconds previously. `ci-observation.json` records exact run/job IDs and per-step measurements. These are separate hosted-run observations and are cache/load-sensitive; they do not establish an overall CI, cold-build or application runtime speedup. The full newer Linux job was still active when these completed steps were inspected.
