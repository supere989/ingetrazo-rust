# Navigation parity workstream

P17 remains incomplete. Source core/camera.py79-118 supplies look_from, clockwise/head-up turn and eye translation using one orbit camera; tools/walkthrough.py defines PositionCamera, LookAround, Walk andFirstPerson operative behavior. Native menus currently onlyreportstatus.

This isolated branch begins native camera primitives without a second camera model. Required remaining work: fresh independent source camera comparisons; actual menu/input routing; default1.68m persisted eyeheight and sensitivity; click/drag PositionCamera handoff; LookAround height/drag; elapsed-time bounded Walk run/Shift/Alt behavior; visible-floor following and eye/knee wall clearance; keyboard FirstPerson capture/release; cancellation/settings; rendered native acceptance and history/persistence checks. No full P17 or release claim is made.
