#!/usr/bin/env python3
"""Actual frozen-original object identity after native Rust publication."""
import json, os, sys, subprocess, hashlib
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
app=QApplication([])
a=Path("docs/audits/mesh-share-codec-review/original-reopen");a.mkdir(parents=True,exist_ok=True)
identity=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
mesh={"edges":[],"faces":[{"vertices":[[0,0,0],[1,0,0],[0,1,0]],"vendor_face":31}],"vendor_mesh":42}
source={"igz_format":1,"app_version":"0.3.15","scene":{"edges":[],"faces":[],"protos":[mesh,mesh],"groups":[{"name":name,"proto":proto,"component":component,"xform":identity,"uid":"placement-"+name,"vendor_placement":i} for i,(name,proto,component) in enumerate([("component",0,True),("plain",0,False),("independent",1,False)])]}}
source["scene"]["protos"].append({"edges":[],"faces":[],"vendor_container":91})
for n in range(2):
 source["scene"]["groups"].append({"name":"parent"+str(n),"proto":2,"component":True,"xform":identity,"uid":"parent"+str(n),"children":[{"name":"child","proto":0,"component":False,"xform":identity,"uid":"child"+str(n),"vendor_child":93+n}]})
(a/"source.igz").write_text(json.dumps(source))
def load(name):
 s=Scene();igz.load_into(s,a/(name+".igz"));return s
original=load("source")
assert original.groups[0].mesh is original.groups[1].mesh
assert original.groups[0].mesh is not original.groups[2].mesh
subprocess.run(["cargo","run","--locked","-p","ingetrazo-formats","--example","mesh_share_acceptance","--",str(a)],check=True)
published=load("published")
assert published.groups[0].mesh is published.groups[1].mesh
assert published.groups[0].mesh is not published.groups[2].mesh
assert [g.mesh.vertices[0].position.z() for g in published.groups[:3]]==[7,7,0]
assert [g.component for g in published.groups[:3]]==[True,False,False]
assert [g.uid for g in published.groups]==[g.uid for g in original.groups]
assert [len(g.children) for g in published.groups[3:]]==[2,1]
assert published.groups[3].children[0].mesh is published.groups[1].mesh
assert published.groups[3].children[1].mesh is published.groups[1].mesh
assert published.groups[4].children[0].mesh is published.groups[1].mesh
assert published.groups[3].children[0].uid==original.groups[3].children[0].uid
assert published.groups[4].children[0].uid==original.groups[4].children[0].uid
raw=json.loads((a/"published.igz").read_text())["scene"]
def no_internal(groups):
 for g in groups:
  assert "_mesh_share" not in g and "_component_definition" not in g
  no_internal(g.get("children",[]))
no_internal(raw["groups"])
unique=load("unique")
assert unique.groups[0].mesh is not unique.groups[1].mesh
assert not unique.groups[0].component
assert [g.mesh.vertices[0].position.z() for g in unique.groups[:3]]==[7,7,0]
report={"passed":True,"source_pin":subprocess.check_output(["git","-C","ingetrazo","rev-parse","HEAD"],text=True).strip(),"candidate":subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip(),"checks":["original source actual object identity","published component-false shared Mesh","equal different prototypes independent","published vertices and flags","placement UID preservation","Make Unique object detachment","independent unequal parent child lists preserve shared child Mesh","internal codec comparison identities stripped"],"files":{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in a.glob("*.igz")},"scope":"Original actual loader and Mesh object identity; no rendered component Finish claim."}
(a/"report.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
