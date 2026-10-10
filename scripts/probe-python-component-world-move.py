#!/usr/bin/env python3
"""Actual original Viewport entry/exit and Move scalar/vector VCB in world edit."""
import os,sys,json
from pathlib import Path
from types import SimpleNamespace as N
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from PySide6.QtCore import QPointF
from views.viewport import Viewport
from core.group import Group,copy_group
from core.mesh import Mesh
from tools.move import MoveTool
from acceptance_context import context
app=QApplication([])
rows=[]
def points(mesh):return [list(v.position.toTuple()) for v in mesh.vertices]
def camera(vp):return {"target":list(vp.camera.target.toTuple()),"eye":list(vp.camera.eye().toTuple()),"distance":vp.camera.distance,"yaw":vp.camera.yaw,"pitch":vp.camera.pitch}
for kind in ("leaf", "container", "nested-leaf"):
 for mode in ("scalar", "vector"):
  vp=Viewport();vp.flash_status=lambda *args:None
  m=Mesh();f=m.add_face([V(0,0,0),V(1,0,0),V(0,1,0)]);f.attrs.update({"vendor":71,"texture":{"uvw":[1.,0.,0.,3.,0.,1.,0.,4.]}})
  g=Group(m,name=kind);g.component=True
  g.xform=QMatrix4x4(-2.,.6,0.,3., 0.,3.,.4,-2., 0.,0.,1.5,1., 0.,0.,0.,1.)
  if kind=="container":g.children=[Group(name="child")]
  sibling=copy_group(g,V(10,0,0));vp.scene.groups=[g,sibling]
  if kind=="nested-leaf":
   parent=Group(name="outer");parent.component=True;parent.xform=QMatrix4x4();parent.xform.translate(2,-1,4);parent.xform.rotate(23,V(0,0,1));parent.children=[g,sibling];vp.scene.groups=[parent];vp.begin_group_edit(parent)
  frame=QMatrix4x4(g.xform);before_camera=camera(vp);before_proto=points(sibling.mesh)
  vp.begin_group_edit(g);entry=points(vp.scene.mesh);assert camera(vp)==before_camera
  grab=V(*entry[0]);hover=grab+V(0,3,0)
  vp.scene.select([vp.scene.mesh.faces[0]]);t=MoveTool();vp.set_active_tool(t)
  t.on_click(N(viewport=vp,world=grab,screen=QPointF(0,0)))
  t.on_hover(N(viewport=vp,world=hover,screen=QPointF(0,0)))
  value=2. if mode=="scalar" else (1.,-2.,.5)
  assert t.on_value(vp,value);assert not vp.history.last_error
  active_after=points(vp.scene.mesh);vp.end_one_group_edit();assert camera(vp)==before_camera
  rows.append({"kind":kind,"mode":mode,"world_frame":list(frame.data()),"entry_world_points":entry,"world_grab":list(grab.toTuple()),"world_hover":list(hover.toTuple()),"vcb_value":value,"active_world_after":active_after,"selected_prototype_after":points(g.mesh),"sibling_prototype_after":points(sibling.mesh),"sibling_prototype_before":before_proto,"selected_matrix":list(g.xform.data()),"sibling_matrix":list(sibling.xform.data()),"selected_attrs_after":g.mesh.faces[0].attrs,"sibling_attrs_after":sibling.mesh.faces[0].attrs,"shared_after":g.mesh is sibling.mesh,"source_history_entries":len(vp.history.undo_stack),"camera_unchanged":camera(vp)==before_camera})
Path("docs/audits/component-world-move-source.json").write_text(json.dumps({"provenance":context(),"scope":"Actual original Viewport.begin_group_edit/end_one_group_edit and Move.on_click/on_hover/on_value with controlled world points. Raw world scalar/vector VCB, inverse prototype shareback, private container ownMesh and fixed camera; no pixel/pointer proof.","cases":rows},indent=2)+"\n")
print("Original world edit Move scalar/vector six controls PASS")
