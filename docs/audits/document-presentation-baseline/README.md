# Operative document presentation baseline defect

Pinned original StylesPanel/ShadowsPanel callbacks mutate full document-owned
settings outside model History. Source controls also exercise ordinary model
Undo/Redo, ScenesPanel capture, save and actual original reopen. Native uses
public app.open_document, model history, style/shadow controls, saved-view capture,
save and native loader. Config and clipboard are isolated.

Confirmed defects:
- Loaded authored source shaded/true is displayed natively as textures/false.
- Editing native display controls and saving preserves stale raw source fields:
  shaded/true instead of wireframe/false.
- Native saved-view capture loses shadow date/time/darkness/UTC and style fields.

Raw original document retention alone does not satisfy operative edit semantics.
The checker uses exact JSON storage values and explicit float32 conversion only
for live GPU-facing color fields, not a widened numeric tolerance. Failed
baseline is mandatory and retained, not an accepted parity verdict.

Next implementation needs modeled full core presentation state, precise source
f64 color retention, document open/save/recovery/context publication bridges and
source nonhistory ownership. Saved-view capture/apply must share the same state.
Actual shadow GPU rendering, full style presets/library/UI and pixels remain
separate full-scope gates. No tile state change.
