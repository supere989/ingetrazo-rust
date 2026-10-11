#!/usr/bin/env python3
"""Pinned material command targets, optional recipe replacement and history."""
import os,sys,json,subprocess,copy
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group,copy_group
from core.history import History,RestampMaterialCommand
from core.materials import Material
PIN="6be29fe437a7cd992d4e079f71720821b25216b9"
def mesh():
 m=Mesh();f=m.add_face([V(0,0,0),V(2,0,0),V(2,1,0),V(0,1,0)])
 f.attrs.update({"mat":"Oak","color":[.1,.2,.3],"texture":{"path":"old.png","sw":2.,"sh":3.,"uvw":[1,0,0,0,0,1,0,0]},"opacity":.4,"back":{"mat":"Oak","color":[.8,.7,.6]},"vendor":{"id":71},"ifc":{"class":"IfcSlab","id":9}})
 return m

def state(s,phase):
 def gs(groups):return [{"name":g.name,"faces":[f.attrs for f in g.mesh.faces],"material":g.material,"children":gs(g.children)} for g in groups]
 return copy.deepcopy({"phase":phase,"registry":{k:v.to_dict() for k,v in s.materials.items()},"active":[f.attrs for f in s.mesh.faces],"loose":[f.attrs for f in s.loose_mesh.faces],"groups":gs(s.groups)})
def main():
 assert subprocess.check_output(["git","-C","/work/ingetrazo","rev-parse","HEAD"],text=True).strip()==PIN
 cases=[]
 for case in ["root","classic","placed","nested"]:
  s=Scene();s.mesh=mesh();s.materials["Oak"]=Material("Oak",color=(.1,.2,.3),texture={"path":"old.png","sw":2.,"sh":3.},opacity=.4,finish="wood")
  g=Group(mesh(),name="target");g.material={"mat":"Oak","color":[.1,.2,.3],"vendor":72}
  if case in ["placed","nested"]:g.xform=M();g.xform.translate(4,5,0)
  sibling=copy_group(g);sibling.name="sibling"
  if case=="nested":p=Group(Mesh(),name="parent");p.adopt([g]);s.groups=[p,sibling]
  else:s.groups=[g,sibling]
  h=History(s)
  if case!="root":s.begin_group_edit(s.groups[0]);
  if case=="nested":s.begin_group_edit(g)
  phases=[state(s,"before")]
  h.execute(RestampMaterialCommand("Oak",Material("Oak",color=(.6,.5,.4),finish="wood")));assert h.last_error is None
  phases.append(state(s,"edited"));h.undo();assert h.last_error is None;phases.append(state(s,"undo"));h.redo();assert h.last_error is None;phases.append(state(s,"redo"))
  if case!="root":
   while s.edit_group is not None:s.end_group_edit()
   phases.append(state(s,"finished"));h.undo();phases.append(state(s,"finished-undo"));h.redo();phases.append(state(s,"finished-redo"))
  cases.append({"case":case,"phases":phases})
 out=Path(sys.argv[1]);out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps({"source_pin":PIN,"cases":cases},indent=2)+"\n")
if __name__=="__main__":main()
