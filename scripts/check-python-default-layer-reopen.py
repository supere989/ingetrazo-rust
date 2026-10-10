#!/usr/bin/env python3
"""Actual frozen Python reload of fresh native named-layer and history files."""
import copy,json,os,sys
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from core.scene import Scene
from core.group import Group
from formats.igz import load_into
p=Path(sys.argv[1]);reference=json.loads((p/"original.json").read_text());cases=[]
for c in reference["cases"]:
 s=Scene();load_into(s,p/("native-"+c["name"]+".igz"));cases.append({"name":c["name"],"names":[l.name for l in s.layers],"default_visible":s.entity_visible(Group())})
assert cases==reference["cases"],"native saved authored registry changed on actual source reopen"
states=[]
for i in range(5):
 s=Scene();load_into(s,p/(f"assignment-{i}.igz"));states.append({"layers":[l.name for l in s.layers],"face":s.mesh.faces[0].attrs.get("layer"),"edge":s.mesh.edges[0].layer,"group":s.groups[0].layer})
assert states==reference["assignment"],"native saved command/history labels changed on actual source reopen"
print(json.dumps({"codec_reopens":len(cases),"assignment_reopens":len(states),"equal":True},indent=2))
