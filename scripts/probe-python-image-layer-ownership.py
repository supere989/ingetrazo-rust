#!/usr/bin/env python3
import sys,os,json
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.viewport import Viewport
from views.tray import LayersPanel
from core.image_plane import ImagePlane
from core.history import AddImagePlaneCommand,MoveImagePlanesCommand
from core.layers import Layer
from core.group import Group
from core.mesh import Mesh
from acceptance_context import context
app=QApplication([]);rows={}
def panel(v):return LayersPanel(SimpleNamespace(viewport=v,statusBar=lambda:SimpleNamespace(showMessage=lambda *a:None)))
def choose(p,name):p.tree.setCurrentItem(next(p.tree.topLevelItem(i)for i in range(p.tree.topLevelItemCount())if p.tree.topLevelItem(i).text(0)==name))
def state(v):return {"layers":[l.name for l in v.scene.layers],"planes":[{"name":p.name,"layer":p.layer,"origin":[p.origin.x(),p.origin.y(),p.origin.z()]}for p in v.scene.image_planes],"undo":len(v.history.undo_stack),"redo":len(v.history.redo_stack)}
for case in ["rename","existing_redo","delete","nested_delete"]:
 v=Viewport();v.flash_status=lambda *a:None
 im=ImagePlane("scan.png",V(),V(1,0,0),V(0,1,0),name="New")
 if case in ["delete","nested_delete"]:
  v.scene.layers.append(Layer("Images"));v.scene.image_planes.append(im)
  mesh=Mesh();mesh.add_face([V(),V(1,0,0),V(0,1,0)]);g=Group(mesh,"Leaf");g.component=True;g.xform=M()
  if case=="nested_delete":
   parent=Group(name="Outer");parent.children=[g];parent.xform=M();v.scene.groups.append(parent);v.begin_group_edit(parent)
  else:v.scene.groups.append(g)
  v.begin_group_edit(g);v.history.execute(MoveImagePlanesCommand([im],V(2,3,4)))
  p=panel(v);choose(p,"Images");p._on_delete();states=[state(v)]
  v.end_one_group_edit()
  if case=="nested_delete":v.end_one_group_edit()
  states.append(state(v));v.history.undo();states.append(state(v));v.history.redo();states.append(state(v))
 else:
  v.history.execute(AddImagePlaneCommand(im));states=[state(v)]
  if case=="rename":
   p=panel(v);choose(p,"Images");p.tree.currentItem().setText(0,"Renamed");states.append(state(v))
   v.history.undo();states.append(state(v));v.history.redo();states.append(state(v));v.history.undo();states.append(state(v))
  else:
   v.history.undo();states.append(state(v));v.scene.layers.append(Layer("Images"));states.append(state(v));v.history.redo();states.append(state(v));v.history.undo();states.append(state(v));v.history.redo();states.append(state(v))
 rows[case]=states
Path(sys.argv[1]).write_text(json.dumps({"provenance":context(),"cases":rows},indent=2)+"\n")
