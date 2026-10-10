#!/usr/bin/env python3
"""Retain original unchanged sharing-leaf Finish followed by Redo."""
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
from core.history import SnapshotMutation
from acceptance_context import context
app=QApplication([]);vp=Viewport();vp.flash_status=lambda *a:None
mesh=Mesh();mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)]);g=Group(mesh,name="component");g.component=True;g.xform=QMatrix4x4();g.xform.translate(3,2,1);peer=copy_group(g,V(10,0,0));vp.scene.groups=[g,peer]
vp.begin_group_edit(g);vp.history.execute(SnapshotMutation(lambda scene:scene.mesh.add_face([V(20,0,0),V(21,0,0),V(20,1,0)])));assert not vp.history.last_error
MainWindow._on_undo(N(viewport=vp));active=vp.scene.mesh;before={"active_faces":len(active.faces),"undo":len(vp.history.undo_stack),"redo":len(vp.history.redo_stack)}
vp.end_one_group_edit();after_exit={"group_faces":len(g.mesh.faces),"peer_faces":len(peer.mesh.faces),"undo":len(vp.history.undo_stack),"redo":len(vp.history.redo_stack),"active_mesh_detached":g.mesh is not active}
MainWindow._on_redo(N(viewport=vp));after_redo={"group_faces":len(g.mesh.faces),"peer_faces":len(peer.mesh.faces),"detached_faces":len(active.faces),"undo":len(vp.history.undo_stack),"redo":len(vp.history.redo_stack)}
assert after_exit["redo"]==1 and after_redo["group_faces"]==after_redo["peer_faces"]==1 and after_redo["detached_faces"]==2
Path("docs/audits/component-finish-redo-source-failure.json").write_text(json.dumps({"provenance":context(),"scope":"Actual original Viewport entry/unchanged exit, operative SnapshotMutation (mesh addition closure), MainWindow MainWindow Undo/Redo; controlled callers, no rendered proof.","before_exit":before,"after_exit":after_exit,"after_redo":after_redo,"literal_original_redo_changes_detached_mesh_only":True},indent=2)+"\n")
print("Retained original unchanged component Finish leaves Redo targeting detached mesh")
