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
s,a,b,f,g,c=setup(); held=[a,b,f,g,c]
a.hidden=True;s.selection={a,b,f,g};w._on_hide();record('hide-filter',held)
w._on_unhide_last();record('last-omits-face',held)
s.selection={a,f,c};w._on_unhide_selected();record('selected-hidden-only',held)
w._on_unhide_selected();record('selected-noop',held)
c.hidden=True;w._on_unhide_all();record('all-nested',held)
w._on_unhide_all();record('all-noop',held)
vp.history.undo();record('all-undo',held);vp.history.redo();record('all-redo',held)
s,a,b,f,g,c=setup();held=[a,b,f,g,c];s.selection={f};w._on_hide();record('face-hide',held)
w._on_unhide_last();record('face-last-noop',held)
s,a,b,f,g,c=setup();held=[a,b,f,g,c];s.selection={b};w._on_hide();s.selection={a};w._on_delete_selection();w._on_unhide_last();record('compacted-edge-last',held)
vp.history.undo();record('compacted-unhide-undo',held);vp.history.undo();record('compacted-delete-undo',held)
s,a,b,f,g,c=setup();held=[a,b,f,g,c];s.selection={b};w._on_hide();s.selection={b};w._on_delete_selection();w._on_unhide_last();record('detached-edge-last',held)
w._on_unhide_last();record('detached-edge-last-noop',held);vp.history.undo();record('detached-last-undo',held);vp.history.undo();record('detached-delete-undo',held)
s,a,b,f,g,c=setup();held=[a,b,f,g,c];s.selection={g};w._on_hide();s.selection={g};w._on_delete_selection();w._on_unhide_last();record('detached-group-last',held);w._on_unhide_last();record('detached-group-last-noop',held)
vp.history.undo();vp.history.undo();record('detached-group-restored',held)
s,a,b,f,g,c=setup();held=[a,b,f,g,c];s.selection={b};w._on_hide();s.selection={b};w._on_delete_selection();vp.history.execute(AddEdgeCommand(V(2,0,0),V(3,0,0)));w._on_unhide_last();record('recreated-edge-distinct',held)
s,a,b,f,g,c=setup();held=[a,b,f,g,c];vp.history.execute(SnapshotCompound([AddEdgeCommand(V(5,0,0),V(6,0,0))]));b.hidden=True;f.attrs['hidden']=True;vp.history.undo();record('snapshot-rewinds-flags',held);vp.history.redo();record('snapshot-redo-flags',held)
s,a,b,f,g,c=setup();held=[a,b,f,g,c];vp.history.execute(AddEdgeCommand(V(5,0,0),V(6,0,0)));b.hidden=True;f.attrs['hidden']=True;vp.history.undo();record('delta-keeps-flags',held)
(out/'original.json').write_text(json.dumps(rows,indent=2)+'\n')
w._saved_version=vp.scene.version;w.close()
