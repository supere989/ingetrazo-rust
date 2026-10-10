#!/usr/bin/env python3
"""Actual original nested representation bake outside command History."""
import os,sys,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from views.viewport import Viewport
from core.group import Group,copy_group
from core.mesh import Mesh
from core.history import AddFaceCommand,MoveGroupCommand
from acceptance_context import context
from views.main_window import MainWindow
from types import SimpleNamespace as N
app=QApplication([]);rows=[]
for inner_container in (False,True):
 for parent_command in ("none","face","move"):
  authored_parent=parent_command!="none"
  vp=Viewport();vp.flash_status=lambda *a:None
  m=Mesh();m.add_face([V(0,0,0),V(1,0,0),V(0,1,0)])
  child=Group(m,name="inner");child.component=True;child.xform=QMatrix4x4();child.xform.translate(2,1,0);child.xform.rotate(17,V(0,1,0))
  if inner_container:child.children=[Group(name="grandchild")]
  sibling=copy_group(child,V(8,0,0));outer=Group(name="outer");outer.component=True;outer.xform=QMatrix4x4();outer.xform.translate(2,-1,4);outer.xform.rotate(23,V(0,0,1));outer.xform.scale(-2,3,1);outer.children=[child,sibling];vp.scene.groups=[outer]
  expected_world=[list((outer.xform.map(child.xform.map(v.position))).toTuple()) for v in child.mesh.vertices]
  vp.begin_group_edit(outer)
  if parent_command=="face":vp.history.execute(AddFaceCommand([V(20,0,0),V(21,0,0),V(20,1,0)]));assert not vp.history.last_error
  if parent_command=="move":vp.history.execute(MoveGroupCommand(child,V(1,0,0)));assert not vp.history.last_error
  before=len(vp.history.undo_stack);vp.begin_group_edit(child);vp.end_one_group_edit();after_inner=len(vp.history.undo_stack);vp.end_one_group_edit();after_outer=len(vp.history.undo_stack)
  assert before==after_inner==after_outer==int(authored_parent)
  if authored_parent:MainWindow._on_undo(N(viewport=vp))
  actual_world=[list((child.xform.map(v.position) if child.xform is not None else v.position).toTuple()) for v in child.mesh.vertices]
  differs=max(abs(a-b) for p,q in zip(actual_world,expected_world) for a,b in zip(p,q))>1e-5
  assert not differs
  rows.append({"axes_after_undo":list(child.axes.data()) if child.axes is not None else None,"world_after_undo":actual_world,"expected_pre_move_world":expected_world,"source_geometry_changed_after_undo":differs,"inner_container":inner_container,"authored_parent":authored_parent,"parent_command":parent_command,"history_before_inner":before,"history_after_inner":after_inner,"history_after_outer":after_outer,"parent_faces":len(outer.mesh.faces),"selected_matrix":list(child.xform.data()),"sibling_matrix":list(sibling.xform.data()),"selected_vertices":[list(v.position.toTuple()) for v in child.mesh.vertices],"sibling_vertices":[list(v.position.toTuple()) for v in sibling.mesh.vertices],"selected_shared_with_sibling":child.mesh is sibling.mesh})
Path("docs/audits/component-nested-bake-undo-source.json").write_text(json.dumps({"provenance":context(),"scope":"Actual original Viewport nested entry/exit with leaf/container no edit and optional parent AddFaceCommand; Undo through actual MainWindow handler after optional parent Move/AddFace and nested no-edit bake. Source inverse Move composition retains baked mesh/axes while restoring world positions; no pointer/pixel evidence.","cases":rows},indent=2)+"\n")
print("Original nested bake Undo six world-preserving controls PASS")
