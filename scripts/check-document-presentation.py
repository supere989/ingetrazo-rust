#!/usr/bin/env python3
"""No tolerances: storage exact; live GPU float colors explicitly float32."""
import json,sys,struct
from pathlib import Path
source=json.loads(Path(sys.argv[1]).read_text());native=json.loads(Path(sys.argv[2]).read_text());errors=[]
def f32(v):return struct.unpack("f",struct.pack("f",v))[0]
for i,(s,n)in enumerate(zip(source["states"],native["states"])):
 for key in ["face_mode","edges","profiles","back_edges","sky","section_fill"]:
  if n.get(key)!=s["style"][key]:errors.append(f"state{i}.{key}")
 for key in ["edge_color","front_color","background"]:
  if n.get(key)!=[f32(x)for x in s["style"][key]]:errors.append(f"state{i}.{key}")
 if n.get("shadow_enabled")!=s["shadows"]["enabled"]:errors.append(f"state{i}.shadow_enabled")
 for key in ["undo","redo"]:
  if n[key]!=s[key]:errors.append(f"state{i}.{key}")
if len(native["states"])!=len(source["states"]):errors.append("state count")
for key in ["style","shadows"]:
 if native.get("saved_raw_"+key)!=source["reopened"][key]:errors.append("saved."+key)
 if native.get("view_"+key)!=source["reopened"]["view_"+key]:errors.append("saved_view."+key)
report={"passed":not errors,"errors":errors,"boundary":"actual original Qt panel/History/save/reopen versus native app.open_document/controller/save; no rendered image or native shadow renderer claim"}
print(json.dumps(report,indent=2))
if "--expect-failure" in sys.argv:assert errors,"baseline unexpectedly accepted"
else:assert not errors
