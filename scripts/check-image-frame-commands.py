#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
source=json.loads(Path(sys.argv[1]).read_text())["cases"]
native=json.loads(Path(sys.argv[2]).read_text());old=json.loads(Path(sys.argv[3]).read_text())
assert set(source)=={"move_relative","rotate_z","rotate_arbitrary","scale_world","scale_axes","scale_zero","transform_partial","transform_redo_capture","detached_move"}
assert native==source,"literal original frame command mismatch (no numerical tolerance)"
assert set(old)=={"move_relative","detached_move"}
for key in old:assert old[key]!=source[key],"literal old ownership failure not rejected"
for field in ["frame","undo","redo"]:
 bad=copy.deepcopy(native);bad["rotate_arbitrary"][1][field]=[]if field=="frame"else 99;assert bad!=source
assert {}!=source
print(json.dumps({"passed":True,"states":sum(map(len,source.values())),"negative_controls":6,"comparison":"exact Python/Qt f32 values and command-held lifetime/history","boundary":"controller commands; interactive dispatch and GPU image planes still open"},indent=2))
