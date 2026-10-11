#!/usr/bin/env python3
"""One missing ancestor creation command exercised while a nested mesh is active."""
import os,sys,json
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow,QInputDialog
from PySide6.QtGui import QVector3D as V
from PySide6.QtCore import QPointF,Qt
from views.viewport import Viewport
from tools.text import TextTool
from formats import igz
out=Path(sys.argv[1]);app=QApplication([]);win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp
igz.load_into(vp.scene,out/'root-face-input.igz');s=vp.scene;tool=TextTool();tool.on_activate(vp);rows=[]
ctx=lambda p:SimpleNamespace(viewport=vp,screen=QPointF(-100,-100),world=V(p),modifiers=Qt.NoModifier)
v=lambda p:[p.x(),p.y(),p.z()]
def record(phase):
 rows.append(dict(phase=phase,labels=[dict(anchor=v(t.anchor),offset=v(t.offset),text=t.text) for t in s.text_labels],label_selection=sorted(s.text_labels.index(t) for t in s.selection if t in s.text_labels),edge_selection=sorted(s.mesh.edges.index(e) for e in s.selection if e in s.mesh.edges),vertices=[v(e.position) for e in s.mesh.vertices],undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),depth=len(s._edit_stack)))
tool.on_click(ctx(V(30,20,0)));old=QInputDialog.getMultiLineText;QInputDialog.getMultiLineText=lambda *a,**k:('created root',True)
try:tool.on_click(ctx(V(31,22,0)))
finally:QInputDialog.getMultiLineText=old
record('created');s.selection.clear();s.selection.add(s.groups[0]);vp.begin_group_edit(s.groups[0]);s.selection.clear();s.selection.add(s.edit_group.children[0]);vp.begin_group_edit(s.edit_group.children[0]);s.selection.clear();s.selection.add(s.mesh.edges[0]);s.selection.add(s.text_labels[0]);s.selection.add(s.text_labels[-1]);record('entered-selected')
vp.history.undo();record('undo');vp.history.redo();record('redo');tool.on_cancel(vp);record('tool-cancel');vp.end_group_edit();record('finish')
(out/'original-history.json').write_text(json.dumps(dict(states=rows),indent=2))
