# Independent presentation parity review

Pinned Python source: 6be29fe437a7cd992d4e079f71720821b25216b9 (version 0.5.7).
Rust workspace package version remains 0.3.15.

The final-runtime replay compares actual original Qt panel/history operations with native controllers: four direct/nested ordinary/folded contexts, 16 states, and five corruptions rejected. The independently executed previous native controller rejects with 35 differences (old-controller-rejection.log).

The final-codec replay compares 16 actual source from_dict cases, including conversion refusals. The actual previous grammar has the same case inventory and fails cases 12, 13, and 15. The checker requires these exact failures, preserving the negative evidence.

The file-codec replay exercises actual original and native IGZ loaders on all 16 cases. Thirteen successful native saves reopen in the actual original runtime with equal style/shadow values; three refused inputs produce no file.

final-runtime/command-7.log records 548 workspace tests passing, two ignored. These tests do not establish rendered style/shadow parity.

saved-view-fixture/replay.log records the corrected acceptance fixture and actual original ScenesPanel deletion followed by empty History Undo/Redo. Deletion remains deleted, advances version once, and creates no history. The native fixture compares the entire after-deletion scene, preserved first view, and empty stack depths. This corrects an obsolete CI assertion; production history policy is unchanged.

Full renderer parity, malformed SavedView partial exception semantics, Unicode-digit syntax, nonfinite/arbitrary-precision JSON boundaries, complete settings/preset controls, GPU execution and release gates remain open.
