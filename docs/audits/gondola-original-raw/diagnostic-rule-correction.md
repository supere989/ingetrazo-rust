# Resolve diagnostic correction

The raw framebuffer counterexamples disprove universal integer half-down rounding. The sample-capture script retains the mismatch count as a diagnostic metric, but no longer asserts this known-false model as acceptance. Its exact original-image reproducibility assertion remains mandatory.

Actual paired GPU replay is the relevant resolve check: all ten extracted RGBA cases matched on the RTX 5090, including the counterexamples. See `../native-rgba-resolve-root/review.json`. This change does not modify production rendering, reference images, or full-model parity gates. Existing failed runs remain retained with their original output.
