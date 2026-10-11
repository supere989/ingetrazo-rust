#!/usr/bin/env python3
"""Actual pinned Qt Select handlers; offscreen owned widgets, no simulated pick method."""
import os,sys,json,importlib.util
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtCore import QPointF,Qt
from PySide6.QtGui import QVector3D
from tools.select import SelectTool
from tools.base import ToolContext
from core.mesh import Edge,Face
from core.group import Group
spec=importlib.util.spec_from_file_location("fixtures","/work/ingetrazo/tests/test_hidden_things_are_clickable.py")
f=importlib.util.module_from_spec(spec);spec.loader.exec_module(f)
rows=[]
def record(vp,name,point,mod=Qt.NoModifier,action="click"):
 px=vp._world_to_pixel(point);tool=SelectTool()
 reference=vp.pick_edge(*px);visible=vp.pick_visible_edge(*px);picked=tool._pick(vp,*px)
 def identity(e):
  if e is None:return None
  for kind,items in [("edge",vp.scene.mesh.edges),("face",vp.scene.mesh.faces),("group",vp.scene.groups)]:
   for i,item in enumerate(items):
    if e is item:return [kind,i]
  raise AssertionError(type(e))
 ctx=ToolContext(vp,point,QPointF(*px),mod,None)
 if action=="double":tool.on_double_click(ctx)
 elif action=="click":tool.on_click(ctx)
 elif action=="hide":
  from core.history import HideCommand
  vp.history.execute(HideCommand(list(vp.scene.selection)))
 elif action=="undo":vp.history.undo()
 rows.append({"name":name,"action":action,"edit":vp.scene.edit_group is not None,"group_hidden":None if vp.scene.edit_group is not None else [g.hidden for g in vp.scene.groups],"active_vertices":[[v.position.x(),v.position.y(),v.position.z()] for v in vp.scene.mesh.vertices],"point":[point.x(),point.y(),point.z()],"pixel":list(px),"matrix":list((vp.camera.projection_matrix()*vp.camera.view_matrix()).data()),"eye":[vp.camera.eye().x(),vp.camera.eye().y(),vp.camera.eye().z()],"reference":identity(reference),"visible_edge":identity(visible),"picked":identity(picked),"selection":sorted([identity(e) for e in vp.scene.selection]),"objects":vp.scene.show_hidden_objects,"geometry":vp.scene.show_hidden_geometry})
vp=f._vp();seam,below=f._box_with_soft_diagonal(vp)
record(vp,"soft-off",f.V(1.2,1,1));vp.scene.show_hidden_geometry=True;vp.scene.version+=1
record(vp,"soft-on",f.V(1,1,1));vp.scene.show_hidden_geometry=False;vp.scene.version+=1
record(vp,"occluded",f.V(1.6,1,0));record(vp,"outside",f.V(-.5,1,0))
record(vp,"shift-remove",f.V(-.5,1,0),Qt.ShiftModifier)
record(vp,"ctrl-add",f.V(-.5,1,0),Qt.ControlModifier)
record(vp,"both-remove",f.V(-.5,1,0),Qt.ShiftModifier|Qt.ControlModifier)
vp=f._vp();g=f._boxed_group(vp);g.hidden=True;vp.scene.version+=1
record(vp,"hidden-object-off",f.V(2,2));vp.scene.show_hidden_objects=True;vp.scene.version+=1
record(vp,"hidden-object-on",f.V(2,2))
vp=f._vp();face=vp.scene.mesh.add_face([f.V(0,0),f.V(2,0),f.V(2,2),f.V(0,2)]);face.attrs={"hidden":True};vp.scene.version+=1
record(vp,"hidden-face-off",f.V(1,1));vp.scene.show_hidden_geometry=True;vp.scene.version+=1
record(vp,"hidden-face-on",f.V(1,1))
vp.scene.layers[0].locked=True;vp.scene.version+=1
record(vp,"hidden-face-locked",f.V(1,1))
vp.scene.layers[0].locked=False;vp.scene.layers[0].visible=False;vp.scene.version+=1
record(vp,"hidden-face-layer-off",f.V(1,1))
vp.scene.layers[0].visible=True;vp.scene.version+=1
record(vp,"hidden-face-layer-restored",f.V(1,1))
from views.main_window import MainWindow
from types import SimpleNamespace
MainWindow._set_hidden_view(SimpleNamespace(viewport=vp),"show_hidden_geometry",False)
record(vp,"geometry-toggle-off",f.V(1,1),action="state")
vp=f._vp();g=f._boxed_group(vp);g.hidden=True;vp.scene.show_hidden_objects=True;vp.scene.version+=1
record(vp,"object-before-toggle",f.V(2,2))
MainWindow._set_hidden_view(SimpleNamespace(viewport=vp),"show_hidden_objects",False)
record(vp,"object-toggle-off",f.V(2,2),action="state")
vp.scene.show_hidden_objects=True;vp.scene.version+=1
record(vp,"object-enter",f.V(2,2),action="double")
vp=f._vp();g=f._boxed_group(vp)
record(vp,"undo-object-before",f.V(2,2))
record(vp,"undo-object-hide",f.V(2,2),action="hide")
MainWindow._set_hidden_view(SimpleNamespace(viewport=vp),"show_hidden_objects",True)
record(vp,"undo-object-shown",f.V(2,2),action="state")
record(vp,"undo-object-undo",f.V(2,2),action="undo")
vp=f._vp();f._box_with_soft_diagonal(vp)
from core.style import Style
vp.scene.display_style=Style(face_mode="xray")
record(vp,"xray-below",f.V(1.6,1,0))
vp.scene.display_style=Style(face_mode="wireframe")
record(vp,"wireframe-below",f.V(1.6,1,0))
vp=f._vp();f._box_with_soft_diagonal(vp)
record(vp,"double-soft",f.V(1.2,1,1),action="double")
vp.scene.show_hidden_geometry=True;vp.scene.version+=1
record(vp,"double-visible",f.V(1,1,1),action="double")
vp=f._vp();f._box_with_soft_diagonal(vp)
from core.layers import Layer
vp.scene.layers.append(Layer("solid",locked=True))
for face in vp.scene.mesh.faces:face.attrs={"layer":"solid"}
vp.scene.version+=1
record(vp,"locked-occluder",f.V(1.6,1,0))
vp.scene.layers[-1].visible=False;vp.scene.version+=1
record(vp,"excluded-occluder",f.V(1.6,1,0))
from pathlib import Path
Path(sys.argv[1]).write_text(json.dumps({"source_commit":"6be29fe437a7cd992d4e079f71720821b25216b9","rows":rows},indent=2)+"\n")
print("Captured",len(rows),"actual Qt Select handler states")
