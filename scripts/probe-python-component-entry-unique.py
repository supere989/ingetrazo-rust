#!/usr/bin/env python3
"""Actual original viewport ordinary copied-group auto-MakeUnique entry policy."""
import os,sys,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from views.viewport import Viewport
from core.group import Group,copy_group
from core.mesh import Mesh
from core.history import DeleteGroupCommand
from acceptance_context import context
app=QApplication([]);rows=[]
for kind in ("root-plain-shared","root-component-shared","root-plain-unshared","classic-plain-shared","nested-plain-parent","nested-component-parent","nested-deleted-sibling"):
 vp=Viewport();vp.flash_status=lambda *a:None
 mesh=Mesh();face=mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)]);face.attrs.update({"vendor":71,"texture":{"uvw":[1.,0.,0.,3.,0.,1.,0.,4.]}})
 g=Group(mesh,name=kind);g.component=kind=="root-component-shared"
 if kind!="classic-plain-shared":g.xform=QMatrix4x4(-2.,.6,0.,3., 0.,3.,.4,-2., 0.,0.,1.5,1., 0.,0.,0.,1.)
 sibling=None
 if kind!="root-plain-unshared":
  sibling=Group(mesh,name="classic sibling") if kind=="classic-plain-shared" else copy_group(g,V(10,0,0))
  sibling.component=g.component
 vp.scene.groups=[g]+([sibling] if sibling else [])
 if kind.startswith("nested"):
  parent=Group(name="outer");parent.component=kind=="nested-component-parent";parent.xform=QMatrix4x4();parent.xform.translate(2,-1,4);parent.children=vp.scene.groups;vp.scene.groups=[parent];vp.begin_group_edit(parent)
 if kind=="nested-deleted-sibling":vp.history.execute(DeleteGroupCommand(sibling));assert not vp.history.last_error
 before_matrix=list(g.xform.data()) if g.xform is not None else None;before_camera=list(vp.camera.target.toTuple());original=g.mesh
 vp.begin_group_edit(g);entry_history=len(vp.history.undo_stack);entry_points=[list(v.position.toTuple()) for v in vp.scene.mesh.vertices];entry_frame=list(vp.scene.drawing_frame.data()) if vp.scene.drawing_frame is not None else None
 vp.end_one_group_edit();exit_history=len(vp.history.undo_stack)
 unique=kind in ("root-plain-shared","nested-plain-parent")
 assert entry_history==exit_history==int(unique)+int(kind=="nested-deleted-sibling")
 assert (g.xform is None)==(unique or kind=="classic-plain-shared")
 assert list(vp.camera.target.toTuple())==before_camera
 rows.append({"kind":kind,"source_unique_on_entry":unique,"before_matrix":before_matrix,"entry_world_points":entry_points,"entry_drawing_frame":entry_frame,"after_matrix":list(g.xform.data()) if g.xform is not None else None,"after_axes":list(g.axes.data()) if g.axes is not None else None,"after_points":[list(v.position.toTuple()) for v in g.mesh.vertices],"after_attrs":g.mesh.faces[0].attrs,"same_original_prototype_after":g.mesh is original,"sibling_same_mesh_after":g.mesh is sibling.mesh if sibling else None,"sibling_in_context_after":sibling in (parent.children if kind.startswith("nested") else vp.scene.groups) if sibling else False,"history_after_entry":entry_history,"history_after_no_edit_exit":exit_history,"camera_unchanged":True})
Path("docs/audits/component-entry-unique-source.json").write_text(json.dumps({"provenance":context(),"scope":"Actual original Viewport begin/end edit, including automatic MakeUniqueCommand before edit mark. Six root/nested/plain/component/placed/classic identity controls; no rendered pointer proof.","cases":rows},indent=2)+"\n")
print("Original viewport copied-group unique-entry seven controls PASS")
