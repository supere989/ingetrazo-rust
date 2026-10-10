#!/usr/bin/env python3
"""Retained operative original Undo inside auto-unique ordinary edit context."""
import os,sys,json
from pathlib import Path
from types import SimpleNamespace as N
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from views.viewport import Viewport
from views.main_window import MainWindow
from core.group import Group,copy_group
from core.mesh import Mesh
from core.history import AddFaceCommand
from acceptance_context import context
app=QApplication([]);vp=Viewport();vp.flash_status=lambda *a:None
mesh=Mesh();mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)]);g=Group(mesh,name="ordinary");g.component=False;g.xform=QMatrix4x4();g.xform.translate(3,2,1);sibling=copy_group(g,V(10,0,0));vp.scene.groups=[g,sibling]
vp.begin_group_edit(g);before={"history":len(vp.history.undo_stack),"scene_mesh_is_edited_group":vp.scene.mesh is g.mesh,"edited_group_shares_sibling":g.mesh is sibling.mesh}
MainWindow._on_undo(N(viewport=vp))
after_undo={"history":len(vp.history.undo_stack),"scene_mesh_is_edited_group":vp.scene.mesh is g.mesh,"edited_group_shares_sibling":g.mesh is sibling.mesh,"edited_context_active":vp.scene.edit_group is g}
vp.history.execute(AddFaceCommand([V(20,0,0),V(21,0,0),V(20,1,0)]));assert not vp.history.last_error
before_exit={"active_mesh_faces":len(vp.scene.mesh.faces),"group_mesh_faces":len(g.mesh.faces),"history":len(vp.history.undo_stack)}
vp.end_one_group_edit();after_exit={"group_mesh_faces":len(g.mesh.faces),"sibling_mesh_faces":len(sibling.mesh.faces),"loose_mesh_faces":len(vp.scene.mesh.faces),"history":len(vp.history.undo_stack)}
assert before["scene_mesh_is_edited_group"] and not after_undo["scene_mesh_is_edited_group"]
assert before_exit["active_mesh_faces"]==2 and before_exit["group_mesh_faces"]==after_exit["group_mesh_faces"]==1
Path("docs/audits/component-entry-undo-source-failure.json").write_text(json.dumps({"provenance":context(),"scope":"Actual original Viewport begin/end edit, MainWindow._on_undo operative handler and AddFaceCommand. Controlled caller, not actual key/pixel. Native context-local Undo policy remains a literal difference; source loses newly authored active face on exit after Undo MakeUnique.","before_undo":before,"after_undo":after_undo,"authored_before_exit":before_exit,"after_exit":after_exit,"source_authored_face_lost_on_exit":True},indent=2)+"\n")
print("Retained actual original auto-unique Undo detached-context loss confirmed")
