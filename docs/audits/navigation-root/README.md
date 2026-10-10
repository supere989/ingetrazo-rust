# Navigation parity workstream

P17 remains incomplete. Source core/camera.py79-118 supplies look_from, clockwise/head-up turn and eye translation using one orbit camera; tools/walkthrough.py defines PositionCamera, LookAround, Walk andFirstPerson operative behavior. Native menus currently onlyreportstatus.

This isolated branch begins native camera primitives without a second camera model. Required remaining work: fresh independent source camera comparisons; actual menu/input routing; default1.68m persisted eyeheight and sensitivity; click/drag PositionCamera handoff; LookAround height/drag; elapsed-time bounded Walk run/Shift/Alt behavior; visible-floor following and eye/knee wall clearance; keyboard FirstPerson capture/release; cancellation/settings; rendered native acceptance and history/persistence checks. No full P17 or release claim is made.

Fresh actual pinned camera comparison:10cases including vertical/zero direction, large turns andtranslations. Maximum absolute difference1.160963e-6 within anexplicit2e-5numeric bound (Qtfloat32/nativefloat64); this is not anexactpixel orfulltool claim. Actualcompiled wrong-turn sign mutation fails; restoredproduction passes. Source/native records, checker andbuildlogs retained.

Native deterministic Walk step now follows source speed/run/dead-zone/turn/Shift/Alt laws, eye+knee clearance, stair/drop/open-ground floorfollowing with a borrowed visible-face ray callback. Seventeen fresh actual Python WalkTool.step cases pass theexplicit2e-5numeric bound (maximum8.768895e-7), using analytic rays; threemeaningful nativecollision/refusalcontrols pass. This doesnot prove live mesh picking, timers/inputrouting orrendered gestures; these remain required before adoption.
