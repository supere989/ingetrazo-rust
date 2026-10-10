#!/usr/bin/env python3
"""Actual original loader of native authored world-edit publications."""
import hashlib,json,os,sys
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
from acceptance_context import context
app=QApplication([]);directory=Path(sys.argv[1]);(directory/"original-verdict.json").unlink(missing_ok=True)
source=json.loads(Path("docs/audits/component-world-move-source.json").read_text());rows=[]
def close(a,b,name):
 assert len(a)==len(b),name
 assert all(abs(x-y)<2e-5 for x,y in zip(a,b)),(name,a,b)
for row in source["cases"]:
 name=row["kind"]+"-"+row["mode"];path=directory/(name+".igz");scene=Scene();igz.load_into(scene,path)
 groups=scene.groups[0].children if row["kind"]=="nested-leaf" else scene.groups
 assert (groups[0].mesh is groups[1].mesh)==row["shared_after"],name
 for i,prefix in enumerate(("selected","sibling")):
  group=groups[i];expected=row[prefix+"_prototype_after"]
  assert len(group.mesh.vertices)==len(expected),name
  for v,e in zip(group.mesh.vertices,expected):close(list(v.position.toTuple()),e,name)
  close(list(group.xform.data()),row[prefix+"_matrix"],name)
  close(group.mesh.faces[0].attrs["texture"]["uvw"],row[prefix+"_attrs_after"]["texture"]["uvw"],name)
 rows.append({"case":name,"passed":True,"sha256":hashlib.sha256(path.read_bytes()).hexdigest()})
report={"passed":True,"provenance":context(),"cases":rows,"scope":"Actual original loader after native world-space authored Move scalar/vector and Finish: prototype points, UV, transforms and Mesh object identity. No pointer/pixel or active Undo claim."}
(directory/"original-verdict.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
