#!/usr/bin/env python3
"""Pinned operative edit, AutoTag, takeoff and history domains; no native fitting."""
import os,sys,json,hashlib,subprocess
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,"ingetrazo")
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from core.scene import Scene
from core.group import Group,copy_group
from core.mesh import Mesh
from core.history import History,AddFaceCommand
from core.edits import build_add_edges
from core.bim import collect_objects,quantities_csv
PIN="6be29fe437a7cd992d4e079f71720821b25216b9"
def rect(s,h,x,y):
 p=[V(x,y,0),V(x+2,y,0),V(x+2,y+1,0),V(x,y+1,0)]
 h.execute(build_add_edges(s,list(zip(p,p[1:]+p[:1])),detect_faces=False,extra=[AddFaceCommand(p)]))
 assert h.last_error is None,h.last_error

def mesh(m):return [{"vertices":[[p.x(),p.y(),p.z()]for p in f.vertices],"holes":[[[p.x(),p.y(),p.z()]for p in h]for h in f.holes],"attrs":f.attrs}for f in m.faces]
def groups(gs):return [{"name":g.name,"faces":mesh(g.mesh),"children":groups(g.children),"transform":None if g.xform is None else list(g.xform.data())}for g in gs]
def snap(s,phase):return {"phase":phase,"active":mesh(s.mesh),"loose":mesh(s.loose_mesh),"groups":groups(s.groups),"rows":[{"class":o["class"],"name":o["name"],"area":o["area"],"volume":o["volume"],"key":o["key"][0]}for o in collect_objects(s)],"csv":quantities_csv(s)}
def main():
 assert subprocess.check_output(["git","-C","/work/ingetrazo","rev-parse","HEAD"],text=True).strip()==PIN
 out=Path("docs/audits/bim-context");out.mkdir(parents=True,exist_ok=True);results=[]
 for name in ["classic-empty-root","classic-root-id40","placed-leaf-root-id40","nested-transformed-root-id40","linked-leaf-root-id40","tagged-mother-root-id40","tagged-group-root-id40"]:
  s=Scene();h=History(s)
  if name!="classic-empty-root":
   f=s.mesh.add_face([V(20,20,0),V(22,20,0),V(22,21,0),V(20,21,0)]);f.attrs["ifc"]={"id":40,"class":"IfcSlab","name":"root"}
  g=Group(Mesh(),name="target")
  if "tagged-group"in name:g.ifc={"class":"IfcSlab","name":"target slab"}
  if "tagged-mother"in name:
   f=g.mesh.add_face([V(0,0,0),V(4,0,0),V(4,4,0),V(0,4,0)]);f.attrs.update({"color":[1,0,0],"vendor":71,"ifc":{"id":99,"class":"IfcSlab","name":"mother"}})
  if "placed"in name or "linked"in name:
   g.xform=M();g.xform.translate(4,5,0)
  if "linked"in name:
   g.component=True;s.groups.append(g);s.groups.append(copy_group(g))
  elif "nested"in name:
   parent=Group(Mesh(),name="parent");parent.adopt([g]);parent.xform=M();parent.xform.translate(4,5,0);g.xform=M();g.xform.translate(2,0,0);s.groups.append(parent)
  else:s.groups.append(g)
  s.active_ifc={"class":"IfcWall","name":"context"}
  s.begin_group_edit(s.groups[0])
  if "nested"in name:s.begin_group_edit(s.groups[0].children[0])
  phases=[snap(s,"entered")];rect(s,h,0,0);phases.append(snap(s,"first"));rect(s,h,0,5);phases.append(snap(s,"second"))
  h.undo();assert h.last_error is None;phases.append(snap(s,"undo"));h.redo();assert h.last_error is None;phases.append(snap(s,"redo"))
  s.end_group_edit();phases.append(snap(s,"finished"))
  results.append({"case":name,"phases":phases})
 (out/"original.json").write_text(json.dumps({"source_pin":PIN,"recipes":results},indent=2)+"\n")
 print(json.dumps({"recipes":len(results),"phases":sum(len(r["phases"])for r in results),"source_pin":PIN}))
if __name__=="__main__":main()
