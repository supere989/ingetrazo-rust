#!/usr/bin/env python3
"""Actual original MainWindow save handlers, unshown Qt widgets, committed edits.
Only native file selection and error-message presentation are intercepted.
"""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMessageBox
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from core import extensions
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.history import History,MoveVerticesCommand
from formats.igz import save_scene,load_into
app=QApplication([]);extensions.plugin_dirs=lambda:[]
from views.main_window import MainWindow,file_dialogs
win=MainWindow();vp=win.viewport
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
def mesh(z):
 m=Mesh();m.add_face([V(0,0,z),V(1,0,z),V(0,1,z)]);return m
def state(s):
 rows=[]
 def visit(g,parent):
  m=parent if g.xform is None else parent*g.xform
  rows.append({'name':g.name,'uid':g.uid,'points':sorted([list(m.map(v.position).toTuple())for v in g.mesh.vertices]),'ext':g.ext})
  for child in g.children:visit(child,m)
 for g in s.groups:visit(g,M())
 return {'root_points':sorted([list(v.position.toTuple())for v in s.mesh.vertices]),'groups':rows}
if len(sys.argv)>2 and sys.argv[2]=='inspect':
 rows={}
 for p in sorted(out.glob('*.native.igz')):
  s=Scene();load_into(s,p);rows[p.stem.removesuffix('.native')]=state(s)
 (out/'reopened.json').write_text(json.dumps(rows,indent=2));win._saved_version=vp.scene.version;win.close();raise SystemExit
s=Scene();s.mesh=mesh(-5)
leaf=Group(mesh(0),'Leaf');leaf.uid='0123456789abcdef';leaf.xform=M();leaf.xform.translate(10,0,0);leaf.ext={'vendor':{'keep':[1,2,3]}}
sibling=Group(leaf.mesh,'Sibling');sibling.uid='fedcba9876543210';sibling.xform=M();sibling.xform.translate(0,10,0)
parent=Group(name='Parent');parent.uid='aaaaaaaaaaaaaaaa';parent.children=[leaf];parent.xform=M();parent.xform.translate(2,3,4);parent.xform.scale(-1,2,1)
s.groups=[parent,sibling];save_scene(s,out/'input.igz')
rows=[]
for case in ['save','save_as_cancel','save_as','failed']:
 s=Scene();load_into(s,out/'input.igz');vp.scene=s;vp.history=History(s);win._current_path=None
 vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0]);win._activate_tool('rectangle')
 vp.history.execute(MoveVerticesCommand([v.position for v in s.mesh.vertices],V(0,0,2)))
 vp.camera.target=V(7,8,9);vp.camera.distance=23
 path=out/(case+'.original.igz');errors=[]
 QMessageBox.critical=lambda *args:errors.append(str(args[-1]))
 dialogs=[]
 def choose(*args):
  dialogs.append(len(s._edit_stack));return ('' if case=='save_as_cancel' else str(path),'')
 file_dialogs.getSaveFileName=choose
 if case in ['save','failed']:
  win._current_path=path if case=='save' else out/'missing'/'failed.igz';win._on_save()
 else:win._on_save_as()
 phases=[]
 for phase in ['after','undo','redo']:
  if phase=='undo':win._on_undo()
  if phase=='redo':win._on_redo()
  phases.append({'phase':phase,'state':state(s),'depth':len(s._edit_stack),'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack),'tool':vp.active_tool.icon,'camera_target':list(vp.camera.target.toTuple()),'camera_distance':vp.camera.distance})
 rows.append({'case':case,'phases':phases,'file_exists':path.exists(),'error':bool(errors),'dialog_depths':dialogs})
(out/'original.json').write_text(json.dumps({'rows':rows},indent=2))
print(json.dumps({'rows':len(rows),'all_exited':all(row['phases'][0]['depth']==0 for row in rows)}))
win._saved_version=vp.scene.version;win.close()
