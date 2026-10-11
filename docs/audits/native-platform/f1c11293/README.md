# f1c11293 actual CI observation

Exact published head `f1c11293b85c1ada5db540eba8e432baa1601f9a`; full run [38168945953](https://github.com/supere989/ingetrazo-rust/actions/runs/38168945953) is terminal **failure** because the two native Mac jobs retain known gates. Linux, Windows and both Apple cross jobs pass. No retries, dispatches, local builds or source captures were performed.

| Job | ID | Result |
|---|---|---|
| Linux workspace/semantic | 114557286544 | PASS |
| Windows compile/process | 114557286545 | PASS |
| ARM cross compile | 114557286478 | PASS |
| Intel cross compile | 114557286577 | PASS |
| ARM native | 114557286443 | Known section/CGL failures |
| Intel native | 114557286530 | Known section/CGL/annotation/CLI failures |

Linux's literal `joint21-result.json` binds all21 executable hashes and source archives. Preview60 states/66 rejects/6 original reopens, anchors35/39+wrong-inverse/5 reopens and Recent Files6/7 pass. The formerly failing late command-specific shareback stage now passes after fixture-only registry refreshes. All Linux semantic stages completed; no giant Linux semantic ZIP was downloaded.

ARM strict section counts remain `[0,0,0,0,0,0,134,152,134,0]`, all cap XOR zero. Intel remains `[292,434,292,576,292,4,1433,1506,1433,436]`, iso cap XOR51. Both accelerated CGL witness setups refuse; Intel strict annotation sample policy and actual CLI remain open. ARM default/profile, billboard, document dimensions and actual packaged CLI pass. These are hosted/paravirtual native results, not physical device qualification or whole-platform parity.

The ordered workflow was **not triggered**: its push path filter excludes this GUI-only batch. No replacement was dispatched. Prior aff ordered run38166916183 remains separate evidence; it is not assigned this head.

`records.zip` authenticates133 members /112,416,979 raw bytes, ZIP SHA `3b832209a7e2329b57ab285377211ac119e0e7d9e5ae8e8ec119f6c574835227`. Every archived member was byte-checked against its downloaded artifact/log record. Original artifact ZIP digests are in observation.json; manifest binds each member. Native archives retain actual build/MSL/runtime and images, Windows isolated recovery records, and cross diagnostics. Source-bindings.json records literal published workflow/runner/fixture git-object hashes; run.json binds actual GitHub head and terminal steps. Owner temporary downloads remain pending parent release.
