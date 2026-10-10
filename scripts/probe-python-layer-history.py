#!/usr/bin/env python3
"""Operative pinned layer panel/command ownership oracle; no desktop I/O."""
import os,sys,json
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import Qt
from PySide6.QtGui import QVector3D as V
from views.viewport import Viewport
from views.tray import LayersPanel
from core.history import AssignLayerCommand,PurgeUnusedCommand,MoveVerticesCommand,SnapshotImport
from core.layers import Layer,layer_of
from acceptance_context import context
app=QApplication([])
def setup():
 v=Viewport();v.flash_status=lambda *a:None;v.scene.mesh.add_edge(V(),V(1,0,0))
 w=SimpleNamespace(viewport=v,statusBar=lambda:SimpleNamespace(showMessage=lambda *a:None))
 return v,LayersPanel(w)
def state(v):
 return {"layers":[[x.name,x.visible,x.locked] for x in v.scene.layers],"edge":layer_of(v.scene.mesh.edges[0]),"undo":len(v.history.undo_stack),"redo":len(v.history.redo_stack)}
def item(p,name):
 return next(p.tree.topLevelItem(i) for i in range(p.tree.topLevelItemCount()) if p.tree.topLevelItem(i).text(0)==name)
rows={}
v,p=setup();v.history.execute(MoveVerticesCommand([V(1,0,0)],V(1,0,0)));p._on_add();i=p.tree.topLevelItem(p.tree.topLevelItemCount()-1);i.setCheckState(1,Qt.Unchecked);i.setCheckState(2,Qt.Checked);a=[state(v)];v.history.undo();a.append(state(v));v.history.redo();a.append(state(v));rows["panel_state_across_model_history"]=a
v,p=setup();v.history.execute(AssignLayerCommand([v.scene.mesh.edges[0]],"Assigned"));p.refresh();item(p,"Assigned").setText(0,"Renamed");a=[state(v)];v.history.undo();a.append(state(v));v.history.redo();a.append(state(v));rows["assign_created_rename_lookup"]=a
v,p=setup();v.scene.layers.append(Layer("Unused"));v.history.execute(PurgeUnusedCommand(layers=True,materials=False));a=[state(v)];v.history.undo();p.refresh();item(p,"Unused").setCheckState(1,Qt.Unchecked);a.append(state(v));v.history.redo();a.append(state(v));v.history.undo();a.append(state(v));rows["purge_held_object_visibility"]=a
v,p=setup();ly=Layer("Imported");v.history.execute(SnapshotImport(lambda s:s.layers.append(ly)));p.refresh();item(p,"Imported").setText(0,"Renamed import");p.tree.setCurrentItem(item(p,"Renamed import"));p._on_delete();a=[state(v)];v.history.undo();a.append(state(v));v.history.redo();a.append(state(v));rows["import_deleted_held_object"]=a
Path(sys.argv[1]).write_text(json.dumps({"provenance":context(),"cases":rows},indent=2)+"\n")
