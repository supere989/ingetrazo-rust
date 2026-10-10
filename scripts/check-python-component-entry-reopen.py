#!/usr/bin/env python3
"""Frozen original loader verifies native UI edit-entry publication."""
import hashlib,json,os,sys
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
from acceptance_context import context
app=QApplication([])
directory=Path(sys.argv[1])
(directory/"original-verdict.json").unlink(missing_ok=True)
source=json.loads(Path("docs/audits/component-entry-unique-source.json").read_text())
rows=[]
for row in source["cases"]:
 kind=row["kind"];path=directory/(kind+".igz");scene=Scene();igz.load_into(scene,path)
 groups=scene.groups[0].children if kind.startswith("nested") else scene.groups
 group=groups[0]
 points=[list(v.position.toTuple()) for v in group.mesh.vertices]
 assert len(points)==len(row["after_points"]),kind
 assert all(abs(a-b)<2e-5 for p,e in zip(points,row["after_points"]) for a,b in zip(p,e)),(kind,points)
 attrs=group.mesh.faces[0].attrs
 # Original loader ignores unknown vendor attrs; verify archive preservation below.
 assert all(abs(a-b)<2e-5 for a,b in zip(attrs["texture"]["uvw"],row["after_attrs"]["texture"]["uvw"])),kind
 if row["source_unique_on_entry"]:
  assert group.xform is None and group.axes is not None,kind
  assert all(abs(a-b)<2e-5 for a,b in zip(list(group.axes.data()),row["after_axes"])),kind
 if row["sibling_in_context_after"]:
  assert (group.mesh is groups[1].mesh)==row["sibling_same_mesh_after"],kind
 raw=json.loads(path.read_text())["scene"]
 selected_raw=raw["groups"][0]["children"][0] if kind.startswith("nested") else raw["groups"][0]
 selected_mesh=raw["protos"][selected_raw["proto"]] if "proto" in selected_raw else selected_raw
 assert selected_mesh["faces"][0]["vendor"]==row["after_attrs"]["vendor"],kind
 def clean(groups):
  for g in groups:
   assert "_mesh_share" not in g and "_component_definition" not in g
   clean(g.get("children",[]))
 clean(raw["groups"])
 rows.append({"kind":kind,"passed":True,"sha256":hashlib.sha256(path.read_bytes()).hexdigest()})
report={"passed":True,"provenance":context(),"cases":rows,"scope":"Actual original loader of native seven UI edit-entry publications; geometry, UV/vendor attributes, classic placement/raw axes and actual Mesh identity. No rendered or active Undo claim."}
(directory/"original-verdict.json").write_text(json.dumps(report,indent=2)+"\n")
print(json.dumps(report,indent=2))
