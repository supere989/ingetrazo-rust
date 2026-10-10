#!/usr/bin/env python3
"""Actual original MainWindow QAction dispatch; no GL/pixel acceptance claim."""
import os,sys,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
os.environ["XDG_CONFIG_HOME"]="/tmp/ingetrazo-history-source-config"
os.environ["XDG_CACHE_HOME"]="/tmp/ingetrazo-history-source-cache"
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from views.main_window import MainWindow
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group,copy_group
from core.history import History,AddFaceCommand
from tools.select import SelectTool
from tools.line import LineTool
from acceptance_context import context
app=QApplication([]);window=MainWindow();vp=window.viewport;vp.flash_status=lambda *a:None
assert window._undo_action.shortcut().toString()=="Ctrl+Z"
rows=[]
def reset(kind):
 vp.scene=Scene();vp.history=History(vp.scene);vp.active_tool=SelectTool()
 mesh=Mesh();mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)])
 g=Group(mesh,name=kind);g.component=kind=="placed-component"
 if kind.startswith("placed"):
  g.xform=QMatrix4x4();g.xform.translate(3,2,1)
 if kind=="container":
  g.xform=QMatrix4x4();g.xform.translate(3,2,1)
  child=Group(Mesh(),name="child");child.mesh.add_face([V(0,0,2),V(1,0,2),V(0,1,2)]);g.children=[child]
 vp.scene.groups=[g];peer=None
 if kind.startswith("placed"):
  peer=copy_group(g,V(10,0,0));vp.scene.groups.append(peer)
 return g,peer,vp.scene.mesh

def state(g,peer,parked):
 return {"undo_commands":[type(c).__name__ for c in vp.history.undo_stack],"redo_commands":[type(c).__name__ for c in vp.history.redo_stack],"active_context":vp.scene.edit_group is g,"active_faces":len(vp.scene.mesh.faces),"group_faces":len(g.mesh.faces),"parked_faces":len(parked.faces),"active_is_group":vp.scene.mesh is g.mesh,"group_shares_peer":g.mesh is peer.mesh if peer else None,"peer_faces":len(peer.mesh.faces) if peer else None}

def add(y):
 vp.history.execute(AddFaceCommand([V(20,y,0),V(21,y,0),V(20,y+1,0)]));assert not vp.history.last_error

for kind in ("classic","container","placed-component"):
 g,peer,parked=reset(kind);vp.begin_group_edit(g);add(0);add(3);after_two=state(g,peer,parked)
 window._undo_action.trigger();after_undo=state(g,peer,parked);assert after_undo["active_faces"]==2 and after_undo["active_context"]
 window._redo_action.trigger();after_redo=state(g,peer,parked);assert after_redo["active_faces"]==3
 vp.end_one_group_edit();after_exit=state(g,peer,parked)
 expected=1 if kind=="placed-component" else 2;assert len(vp.history.undo_stack)==expected
 window._undo_action.trigger();exit_undo=state(g,peer,parked)
 assert exit_undo["group_faces"]==(1 if kind=="placed-component" else 2)
 window._redo_action.trigger();exit_redo=state(g,peer,parked);assert exit_redo["group_faces"]==3
 rows.append({"name":kind+"-local-two-undo-redo-exit","after_two":after_two,"active_undo":after_undo,"active_redo":after_redo,"exit":after_exit,"exit_undo":exit_undo,"exit_redo":exit_redo})
for kind in ("classic","container","placed-component"):
 g,peer,parked=reset(kind);add(0);vp.begin_group_edit(g);active=vp.scene.mesh
 window._undo_action.trigger();undone=state(g,peer,parked);assert len(parked.faces)==0 and vp.scene.mesh is active and vp.scene.edit_group is g
 window._redo_action.trigger();redone=state(g,peer,parked);assert len(parked.faces)==1 and vp.scene.mesh is active
 vp.end_one_group_edit();assert len(vp.history.undo_stack)==1
 rows.append({"name":kind+"-undo-prior-loose-command","active_undo":undone,"active_redo":redone,"exit":state(g,peer,parked)})
g,peer,parked=reset("placed-ordinary");vp.begin_group_edit(g);window._undo_action.trigger();detached=state(g,peer,parked)
assert detached["active_context"] and not detached["active_is_group"]
add(0);authored=state(g,peer,parked);vp.end_one_group_edit();lost=state(g,peer,parked)
assert authored["active_faces"]==2 and lost["group_faces"]==1
rows.append({"name":"ordinary-entry-undo-detached-authored-face-loss","after_entry_undo":detached,"authored_before_exit":authored,"exit":lost,"literal_original_data_loss":True})
g,peer,parked=reset("classic");add(0);vp.begin_group_edit(g);line=LineTool();line.start_point=V(3,4,0);line.chain_first_point=line.start_point;line.chain_vertices=[line.start_point];vp.active_tool=line
window._undo_action.trigger();assert line.start_point is None and len(vp.history.undo_stack)==1 and len(parked.faces)==1
rows.append({"name":"line-first-point-consumes-undo-before-global-history","state":state(g,peer,parked),"chain_reset":line.start_point is None})
# Real Qt key event through the actual MainWindow QAction shortcut, separate
# from the eight deterministic QAction-trigger controls. No GPU/pixel claim.
from PySide6.QtCore import Qt
from PySide6.QtTest import QTest
g,peer,parked=reset("classic");add(0);vp.begin_group_edit(g);active=vp.scene.mesh
window.show();window.activateWindow();vp.setFocus();app.processEvents()
QTest.keyClick(vp,Qt.Key_Z,Qt.ControlModifier);app.processEvents()
assert len(vp.history.undo_stack)==0 and len(parked.faces)==0 and vp.scene.mesh is active and vp.scene.edit_group is g
rows.append({"name":"actual-qt-ctrl-z-prior-loose-command-active-context","state":state(g,peer,parked),"key_event":"Ctrl+Z to real Viewport widget","pixel_acceptance":False})
Path("docs/audits/component-active-history-source.json").write_text(json.dumps({"provenance":context(),"scope":"Actual original MainWindow constructor QAction Ctrl+Z/Redo connections, QAction.trigger through operative handlers, actual Viewport entry/exit and History commands. Qt offscreen; one real Qt Ctrl+Z key event plus eight deterministic QAction controls; no physical input or rendered gesture acceptance. Nine distinct history controls retain original face-loss counterexample.","undo_shortcut":window._undo_action.shortcut().toString(),"cases":rows},indent=2)+"\n")
print("Actual original QAction active-context history nine controls PASS; unsafe source face-loss retained")
window.deleteLater()
