# Navigation parity workstream

P17 remains incomplete. Source core/camera.py79-118 supplies look_from, clockwise/head-up turn and eye translation using one orbit camera; tools/walkthrough.py defines PositionCamera, LookAround, Walk andFirstPerson operative behavior. Native menus currently onlyreportstatus.

This isolated branch begins native camera primitives without a second camera model. Required remaining work: fresh independent source camera comparisons; actual menu/input routing; default1.68m persisted eyeheight and sensitivity; click/drag PositionCamera handoff; LookAround height/drag; elapsed-time bounded Walk run/Shift/Alt behavior; visible-floor following and eye/knee wall clearance; keyboard FirstPerson capture/release; cancellation/settings; rendered native acceptance and history/persistence checks. No full P17 or release claim is made.

Fresh actual pinned camera comparison:10cases including vertical/zero direction, large turns andtranslations. Maximum absolute difference1.160963e-6 within anexplicit2e-5numeric bound (Qtfloat32/nativefloat64); this is not anexactpixel orfulltool claim. Actualcompiled wrong-turn sign mutation fails; restoredproduction passes. Source/native records, checker andbuildlogs retained.
