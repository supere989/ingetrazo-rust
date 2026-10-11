#!/usr/bin/env python3
"""Pinned actual MainWindow Zoom Extents; no displayed-window claim."""
import os,sys,json,subprocess
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
import numpy as np
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.camera import OrbitCamera
from views.main_window import MainWindow
from formats.igz import save_scene as save
from georef.photomesh import PhotoMesh
app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
PIN="6be29fe437a7cd992d4e079f71720821b25216b9"
assert subprocess.check_output(["git","-C","/work/ingetrazo","rev-parse","HEAD"],text=True).strip()==PIN
def state(c):return {"target":[c.target.x(),c.target.y(),c.target.z()],"distance":c.distance,"yaw":c.yaw,"pitch":c.pitch,"perspective":c.perspective,"two_point":c.two_point}
def mesh():
 m=Mesh();m.add_face([V(-2,-1,0),V(4,-1,0),V(4,3,2),V(-2,3,2)]);return m
names=["parallel","perspective","two-point","portrait","hidden-nested","empty","orphan","figure-only","survey-only","survey-hidden","hidden-wire","hidden-face"]
rows=[]
if "--only-missing" in sys.argv:
 rows=json.loads((out/"source.json").read_text())["cases"];names=names[len(rows):]
for name in names:
 s=Scene();c=OrbitCamera();c.target=V(2.25,-1.5,.75);c.distance=31.;c.yaw=-.71;c.pitch=.38;c.aspect=.6 if name=="portrait" else 4/3;c.perspective=name not in ["parallel","hidden-nested"];c.two_point=name=="two-point"
 if name in ["parallel","perspective","two-point","portrait"]:s.mesh=mesh()
 if name=="hidden-nested":
  parent=Group(Mesh(),name="parent");parent.xform=M();parent.xform.translate(6,-2,3);child=Group(mesh(),name="child");child.xform=M();child.xform.scale(-2,1,1);parent.adopt([child]);s.groups=[parent];far=Group(mesh(),name="hidden");far.xform=M();far.xform.translate(900,0,0);far.hidden=True;s.groups.append(far)
 if name=="orphan":s.mesh.vertex(V(500,500,500))
 if name=="figure-only":
  g=Group(mesh(),name="figure");g.billboard=True;g.xform=M();g.xform.translate(6,2,1);s.groups=[g]
 if name.startswith("survey"):
  s.photo_mesh=PhotoMesh(np.array([[-5,-4,-2],[8,9,3],[0,0,0]],dtype=np.float32),np.zeros((3,2),np.float32),np.array([[0,1,2]],dtype=np.int32),[]);s.photo_mesh.visible=name=="survey-only"
 if name=="hidden-wire":
  s.mesh=mesh();e=s.mesh.add_edge(V(20,10,4),V(30,20,8));e.hidden=True
 if name=="hidden-face":
  s.mesh=mesh();f=s.mesh.add_face([V(20,10,4),V(30,10,4),V(30,20,8),V(20,20,8)]);f.attrs["hidden"]=True
 initial={**state(c),"aspect":c.aspect,"fov_deg":c.fov_deg,"znear":c.znear,"zfar":c.zfar};save(s,out/(name+".igz"));updates=[0]
 viewport=SimpleNamespace(scene=s,camera=c,update=lambda:updates.__setitem__(0,updates[0]+1));owner=SimpleNamespace(viewport=viewport);owner._figure_bounds=lambda:MainWindow._figure_bounds(owner)
 MainWindow._on_zoom_extents(owner);once=state(c);MainWindow._on_zoom_extents(owner)
 rows.append({"name":name,"initial":initial,"once":once,"twice":state(c),"updates":updates[0]})
from views.viewport import Viewport
from PySide6.QtGui import QAction
w=Viewport();init=rows[1]["initial"]
for key,value in init.items():setattr(w.camera,key,V(*value) if key=="target" else value)
mode_rows=[{"phase":"before","camera":state(w.camera)}]
for phase,slot in [("two-point",w.toggle_two_point),("parallel",w.toggle_projection),("perspective",w.toggle_projection)]:
 action=QAction(phase,w);action.triggered.connect(slot);action.trigger();mode_rows.append({"phase":phase,"camera":state(w.camera)})
w.close()
(out/"source.json").write_text(json.dumps({"source_pin":PIN,"cases":rows,"mode_session":{"initial":init,"phases":mode_rows}},indent=2)+"\n")
