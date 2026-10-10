# CPU image-outline journal and immutable chunks

This stage adds an allocation-free 64-entry pose journal to the restricted pose setters and a CPU-only per-plane immutable outline producer. It does not replace the live GPU callback or integrate a GPU arena. Source geometry uses the previously independently validated Qt OpenGL strips. No resources files changed.

Focused core tests: 2 pass. Focused GUI tests: 4 pass. The 50,000-plane test performs 2,000 genuinely varying updates in 157,427 microseconds on this run, with zero additional plane-list scans, 2,000 chunk rebuilds, and 125,984 bounded journal records examined. Unchanged visible chunks keep their Arc identity. Changed chunks allocate immutable geometry; this is not a zero-allocation producer or whole-render-path performance claim.

Negative controls cover stale/overflow journals, structural edits, clone/reload ownership, forgotten restricted guards, nonfinite later candidates, and serial exhaustion. Candidate refusal preserves all published slots and publication metadata. Selection/layer changes follow separate invalidation paths. Initial focused compilation failed on a test-only layer module typo, then corrected; earlier submodule-less build lacked embedded source resources. Neither is reported as model acceptance.

Commands use debug=0, incremental=0, -j2 and focused library filters. Terminal target and temporary logs are purged after committing this evidence. Native GPU arena ranges, safe fenced retirement, full callback metrics and native Metal consumption remain required followups.
