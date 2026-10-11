# Integrated hover Delete workflow

Runtime `f7ce50c2` builds the CLI and existing annotation Delete consumer together in **4.99 seconds**. The affected replay/checker/original reader takes **2.14 seconds**: 30 retained Delete states plus 26 hover-Delete states match exactly, all 56 mutations reject, and four original-reader reopens plus four lossy controls pass. No original capture or broad local suite was repeated.

Actual Delete-key dispatch keeps extension-light and real-selection precedence. With empty selection it resolves the validated hovered annotation, clears hover and submits an explicit target without temporarily selecting it. Root/nested Undo/Redo and Finish preserve empty-selection history and source identities. [Owner proof](../annotation-hover-delete/README.md).

Root prereview corrected the extended checker to emit one combined JSON result, preserving its consuming runner contract. Two new authenticated inputs join the existing Delete consumer in the same nineteen-binary CI batch; no extra named binary or Cargo invocation is added. The receipt binds integrated source, checker/runner and both binaries. Every ZIP member was byte-compared to raw proof before retention.

The published `9ee304c6` CI covers the prior hover batch. This later integrated Delete change awaits the next stable publication. P15 remains Implemented: a local-registry live-anchor admission discrepancy and broader glyph/occlusion acceptance remain open. No new GPU or physical input claim is made.
