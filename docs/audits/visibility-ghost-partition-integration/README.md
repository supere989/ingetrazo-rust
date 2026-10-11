# Integrated visibility, Line history and partition acceptance

Native production changes through `7bed16d5`, compiled together at `a96bb645120a3720c3c831d445f644d767e9d8ee`, pass the combined semantic and actual Vulkan acceptance. The original oracle is unchanged and clean at `6be29fe437a7cd992d4e079f71720821b25216b9`. This is a bounded integration checkpoint, not full release parity.

[Semantic acceptance](semantic-acceptance.json) records 883 unique passing tests, two existing ignores and zero remaining failures. The initial broad run found one stale nested Line expectation: bare source AddEdge Undo retains endpoint objects. Only that test expectation changed. Its failure and correction are both retained, and the failed test plus unfinished targets resumed; no other completed test ran twice. Doctests and all examples check passed. The ghost-switch nested/history regression also passed.

The twelve named consumers built together once in 2.12 seconds using the reused target. This is a cached build observation, not a runtime or cold-build speedup claim. All following consumers passed: eleven lifecycle workflows, three pending gestures, 41 phases and four original reopens; nine Line command/Undo inventories and five full bare histories; five valid partition cases and nested/shared history; 60 saved-view states; 34 Hide/Unhide states; prior twelve endpoints; mip archive check; component and world-bake acceptance. Strict checkers retain their corruption controls.

[Fresh integrated GPU acceptance](gpu-acceptance.json) records twelve exact whole RGBA comparisons against the retained original NVIDIA captures, and 21 actual native app actions. These include Objects/Geometry toggles, saved-view capture/update/recall, Unhide/Undo with live flags, refusal preserving the prior image, retry, warm reuse, resize/restore and final Save/clean close. Final GPU ledger is zero. The app harness invokes production methods; it does not prove mouse gestures. The source frames were not recaptured. Root separately replayed the read-only archive checker.

[Stage records](stages.json), [semantic archive manifest](semantic-manifest.json) and [GPU manifest](gpu/manifest.json) retain exact source/binary hashes. `semantic-evidence.zip` contains all semantic logs, test roster, inputs/results and initial failure. `gpu/captures.zip` contains the authenticated original inputs plus fresh native frames and actual app evidence. Every archived byte was checked before temporary cleanup. The prior owner captures and setup failures remain in [the source workflow audit](../hidden-ghost-workflow/README.md).

Remaining scope is explicit: three compound Line forward raw-order differences, a source-accepted degenerate annular collapse with native atomic refusal, broader Push/Pull limits/Ctrl behavior, actual soft/occluded-edge Select clicks, native Metal acceptance and all five release gates. Feature inventory remains 10 Implemented / 27 Partial / 0 Verified.

To rerun only the retained GPU comparison:

```sh
/tmp/ingetrazo-parity-python/bin/python scripts/check-hidden-ghost-acceptance.py docs/audits/visibility-ghost-partition-integration/gpu
```

`execute.py` records each semantic stage and has an explicit build boundary. The historical run used `--stop-after examples-check`, resumed the corrected test and unfinished stages with `--resume-line-test --stop-after examples-check`, then `--start-at named-build`. `replay.sh` is the bounded host GPU replay transport; pass the actual compiled source SHA and a fresh output directory. Rebuild/rerender only for changed code, failures or unresolved concerns.
