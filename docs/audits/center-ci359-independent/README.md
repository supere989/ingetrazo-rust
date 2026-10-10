Two distinct public CI failures remain unresolved.

942 run38032137378: native curve passes; original curve reaches moved-center.png, then times out line156 after Ctrl+A/Delete during center invalidation. 359 run38032281082: native curve reaches center-hint.png then times out line142 awaiting Move undo1. Prior uploader omitted state.json and tracked success reports could survive failure. Those reports are not fresh evidence.

848d587 preserves original input timing and gestures, emits selection/tool/anchor phase snapshots plus failed current state/ownedwindow screenshot, clears old report beforelaunch and writes passed:false onfailure, uploads allstate/failure JSON. A separate latent originaltiny KeyError fromlost center_hint_observation was reproduced, frozen, and corrected by restoring read-only sourceadmission observation. Fresh curve native/original and tiny/malformed controls pass locally, which does not establish the cloud race cause or greenCI.
