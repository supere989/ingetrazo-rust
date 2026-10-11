# Units integration CI repair

Published `562a301d` exposed twelve Linux workspace failures. [The complete original failure and terminal platform record](../native-platform/562a301d/README.md) is retained. The twenty-two joint semantic consumers and later shareback stage were skipped after that failure; the previous local integration pass is not a substitute for those stages.

The one production correction preserves absent unit overrides as `None` when entering component edit. Previously `Some(Null)` propagated through Finish/Cancel/history and changed nine complete scene comparisons. Imported valid units still carry into the editing context, and explicit preference overrides remain live across history.

Three fixture corrections preserve the source contract. The literal pinned Python parser rejects Move `0,2,0` and Rectangle `5.0, 2.0`; a four-row source capture confirms those refusals and the accepted semicolon replacements. Both tests retain the old input as a rejection control. The Push/Pull label fixture now selects millimetres through actual `apply_model_units`, keeping the modeled unit and effective pair coherent instead of forging only raw metadata.

One combined build of the four affected test binaries at `1f4bc75e` passed in **8.97 seconds**. All twelve exact previously failing test names pass in **0.29 seconds**, each with a nonzero one-test receipt. There was no broad local suite or repeated successful consumer run. No acceptance assertion was weakened; whole scene and source geometry assertions remain.

`records.zip` retains the named test outputs, Cargo JSON/build logs, binary hashes, source parser controls and timing. Every member was compared with raw files; all four changed source hashes match build-head Git objects. The next corrected published batch owns broad CI and the previously skipped semantic consumers.
