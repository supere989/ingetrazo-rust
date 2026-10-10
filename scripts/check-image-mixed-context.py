#!/usr/bin/env python3
"""Strict operative mixed-context state comparison; old failures remain required."""
import json,sys,copy
from pathlib import Path
source=json.loads(Path(sys.argv[1]).read_text())["cases"]
native=json.loads(Path(sys.argv[2]).read_text())
old=json.loads(Path(sys.argv[3]).read_text())
required={f"{scope}:{case}" for scope in ["plain","component","folded","nested","nested_folded"] for case in ["finish_live","cancel_then_finish","finish_coincident_root"]}
assert set(source)==set(native)==required,"mandatory fifteen-case corpus missing"
assert source==native,"operative state mismatch"
for scope in ["plain","component","folded","nested","nested_folded"]:
 key=scope+":finish_live"
 assert old[key]!=source[key],"literal old live-Finish failure disappeared"
for field in ["origin","geometry","active","undo"]:
 changed=copy.deepcopy(native);row=changed["component:finish_coincident_root"][2]
 if field=="origin":row[field][0]+=1
 elif field=="geometry":row[field][0][0]+=1
 elif field=="active":row[field]=not row[field]
 else:row[field]+=1
 assert changed!=source,"negative control not rejected"
print(json.dumps({"cases":15,"states":sum(map(len,native.values())),"old_failures":5,"negative_controls":4,"exact":True}))
