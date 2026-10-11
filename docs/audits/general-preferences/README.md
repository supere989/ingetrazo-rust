# General preferences consumer

Actual pinned PreferencesDialog OK/Cancel, isolated QSettings and source History are compared with the actual native App/restart consumer: eight typed states and nine rejecting controls pass. Actual egui Modal checkbox/OK/Cancel/Escape admission passes one focused test. Cancel preserves settings bytes, timer, model and history; accepted changes rearm autosave. General undo cap zero is unlimited, and lowering a positive cap trims on the next command only.

Original ComposerHistory (core/composition.py1768–1815) has no General cap. Native independent sheet history now likewise retains all commands. The existing sheet gesture test passes with General caps zero/one and three complete Undo/Redo operations; the first incorrectly qualified test filter ran zero tests and is retained alongside the corrected one-test result.

Run the prebuilt `context_document_lifecycle <directory> --preferences`, then `python3 scripts/check-general-preferences.py <directory>`. Authenticate/extract records.zip using manifest.json first. The initial dialog-reopen protocol is retained; the final source protocol constructs a new window and reads persisted settings on restart.

Cold opening clones only small preferences; warm paints retain the draft, with no Scene clone or persistence. OK writes once. Existing model-display controls remain staged compatibility controls, without claiming original default-unit completion. Theme, MSAA, toolbar, backup, import, AI, 3D mouse, platform, default units and shortcuts remain outside this General increment. P33 is not fully complete.
