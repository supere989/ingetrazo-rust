# Refused Finish preserves transient tool state

Source/global history review found Finish retired the active tool before candidate and historical snapshot validation. The retained pre-fix test fails because a refused Finish changed Move into Select and discarded its live preview.

Finish now builds an immutable candidate from a cloned Scene with Tool.restore_transient_model applied, validates all lifted history and parked ancestor representations, then retires the tool only after validation succeeds. No failed candidate changes live Scene, context, history, tool/anchor/preview or model revision. Successful Finish still cancels an uncommitted live Move rather than authoring that preview.

Two additional regressions cover invalid current/historical geometry during active Move and successful retirement without new authored geometry/history. The dedicated history suite passes 14 tests. Fresh combined workspace totals are reported with the commit. The frozen original checker still passes 18 observations plus two explicitly bounded native integrity controls. component-finish-atomic-source-check.json records its exact fresh executable SHA. Earlier rendered positive captures remain historical evidence from the pre-fix executable; this narrowly fixes refusal timing and makes no new rendered or broader parity claim.
