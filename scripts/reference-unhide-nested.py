#!/usr/bin/env python3
"""Operative pinned MainWindow Hide/Unhide actions and identity-owned history."""
import json, os, sys
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM', 'offscreen')
sys.path.insert(0, str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QSettings
from PySide6.QtGui import QVector3D as V
from core import extensions
from core.scene import Scene
from core.group import Group
from core.mesh import Mesh
from core.history import History, DeleteEdgesCommand, DeleteGroupCommand, AddEdgeCommand, SnapshotCompound, _is_hidden
app = QApplication([])
extensions.plugin_dirs = lambda: []
QSettings().setValue('new_document/scale_figure', '0')
from views.main_window import MainWindow
w = MainWindow(); vp = w.viewport
out = Path(sys.argv[1]); out.mkdir(parents=True, exist_ok=True)
rows = []
def setup():
 s = Scene(); vp.scene = s; vp.history = History(s); w._saved_version = s.version
 a=s.mesh.add_edge(V(0,0,0), V(1,0,0)); b=s.mesh.add_edge(V(2,0,0), V(3,0,0))
 f=s.mesh.add_face([V(0,2,0), V(1,2,0), V(0,3,0)])
 g=Group(name='parent'); c=Group(name='child'); g.children=[c]; s.groups=[g]
 return s,a,b,f,g,c

def record(label, held):
 s=vp.scene
 rows.append({'label':label, 'held_hidden':[_is_hidden(e) for e in held],
  'held_live':[e in s.mesh.edges or e in s.mesh.faces or e in s.groups or any(e in g.children for g in s.groups) for e in held],
  'edge_hidden':[e.hidden for e in s.mesh.edges], 'face_hidden':[_is_hidden(f) for f in s.mesh.faces],
  'group_hidden':[[g.hidden,[c.hidden for c in g.children]] for g in s.groups],
  'selected':[held.index(e) for e in s.selection if e in held],
  'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack)})
s,a,b,f,g,c=setup()
g.mesh.add_face([V(0,0,5),V(1,0,5),V(0,1,5)])
c.mesh.add_face([V(0,0,6),V(1,0,6),V(0,1,6)])
a.hidden=True;f.attrs['hidden']=True;c.hidden=True
root_face=f;parent_face=g.mesh.faces[0];child_face=c.mesh.faces[0]
def nested(label):
 rows.append({'label':label,'depth':len(s._edit_stack),'flags':[_is_hidden(e)for e in[a,root_face,g,c,parent_face,child_face]],'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack),'selected':len(s.selection)})
s.begin_group_edit(g);w._on_unhide_all();nested('parent-all-active-only')
s.selection={parent_face};w._on_hide();nested('parent-hide-face');w._on_unhide_last();nested('parent-last-face-noop')
vp.history.undo();nested('parent-hide-undo');vp.history.undo();nested('parent-all-undo');vp.history.redo();nested('parent-all-redo')
parent_face.attrs['hidden']=True;child_face.attrs['hidden']=True
s.begin_group_edit(c);w._on_unhide_all();nested('leaf-all-active-only');vp.history.undo();nested('leaf-all-undo');vp.history.redo();nested('leaf-all-redo')
s.end_group_edit();nested('finished-global-flags')
(out/'original-nested.json').write_text(json.dumps(rows,indent=2)+'\n');w._saved_version=s.version;w.close()
