#!/usr/bin/env python3
"""Original unshown MainWindow lifecycle; real Qt prompts with scheduled button clicks."""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMessageBox
from PySide6.QtCore import QSettings,QTimer,QPointF,Qt
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M,QCloseEvent
from core import extensions
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.history import History,MoveVerticesCommand
from formats.igz import save_scene,load_into
from tools.base import ToolContext
from core.snap import SnapResult
app=QApplication([]);extensions.plugin_dirs=lambda:[]
QSettings().setValue('new_document/scale_figure','0')
from views.main_window import MainWindow,file_dialogs
win=MainWindow();vp=win.viewport
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
def mesh(z):
 m=Mesh();m.add_face([V(0,0,z),V(1,0,z),V(0,1,z)]);return m
def xyz(v):return list(v.toTuple())
def mesh_state(mesh,m):
 points=lambda vs:[xyz(m.map(v))for v in vs]
 def ring(vs):
  ps=points(vs);return min(ps[i:]+ps[:i]for i in range(len(ps)))
 return {'points':sorted(points([v.position for v in mesh.vertices])),
 'edges':sorted([sorted(points([e.a,e.b]))for e in mesh.edges]),
 'faces':sorted([{'outer':ring(f.vertices),'holes':sorted([ring(h)for h in f.holes]),'attrs':f.attrs}for f in mesh.faces],key=lambda f:json.dumps(f,sort_keys=True))}
def state(s):
 rows=[]
 def visit(g,parent):
  m=parent if g.xform is None else parent*g.xform
  rows.append({'name':g.name,'uid':g.uid,'mesh':mesh_state(g.mesh,m),'ext':g.ext})
  for child in g.children:visit(child,m)
 for g in (s.edit_group.children if s.edit_group is not None else s.groups):visit(g,M())
 return {'root_mesh':mesh_state(s.mesh,M()),'groups':rows}
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
s=Scene();s.mesh=mesh(31);s.camera_home={'target':[3,4,5],'distance':19};save_scene(s,out/'replacement.igz')
(out/'corrupt.igz').write_bytes(b'invalid zip')
def setup():
 s=Scene();load_into(s,out/'input.igz');vp.scene=s;vp.history=History(s);win._current_path=None;win._saved_version=s.version
 vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0]);win._activate_tool('rectangle')
 vp.history.execute(MoveVerticesCommand([v.position for v in s.mesh.vertices],V(0,0,2)))
 vp.camera.target=V(7,8,9);vp.camera.distance=23
 return s
def snapshot(phase):
 return {'phase':phase,'state':state(vp.scene),'depth':len(vp.scene._edit_stack),'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack),'tool':vp.active_tool.icon,'camera_target':xyz(vp.camera.target),'camera_distance':vp.camera.distance}
rows=[];real_exec=QMessageBox.exec
cases=['new_cancel','open_cancel','open_picker_cancel','open_discard','open_failed','new_discard','close_cancel','close_discard','close_save','close_save_cancel','close_save_failed']
for case in cases:
 s=setup();prompts=[];dialogs=[];errors=[]
 choice=QMessageBox.Cancel if case in ['new_cancel','open_cancel','close_cancel'] else QMessageBox.Save if case.startswith('close_save') else QMessageBox.Discard
 def prompt(box):
  prompts.append(len(s._edit_stack));QTimer.singleShot(0,lambda:box.button(choice).click());return real_exec(box)
 QMessageBox.exec=prompt;QMessageBox.critical=lambda *args:errors.append(str(args[-1]))
 def choose_open(*args):
  dialogs.append(len(s._edit_stack));return ('' if case=='open_picker_cancel' else str(out/('corrupt.igz' if case=='open_failed' else 'replacement.igz')),'')
 file_dialogs.getOpenFileName=choose_open
 def choose_save(*args):dialogs.append(len(s._edit_stack));return ('','')
 file_dialogs.getSaveFileName=choose_save
 path=out/(case+'.original.igz')
 if case=='close_save':win._current_path=path
 elif case=='close_save_failed':win._current_path=out/'missing'/'failed.igz'
 accepted=False
 if case.startswith('new'):win._on_new()
 elif case.startswith('open'):win._on_open()
 else:
  event=QCloseEvent();win.closeEvent(event);accepted=event.isAccepted()
 prompt_open=any(isinstance(w,QMessageBox)and w.isVisible()for w in app.topLevelWidgets())
 phases=[snapshot('after')]
 if not accepted:
  win._on_undo();phases.append(snapshot('undo'));win._on_redo();phases.append(snapshot('redo'))
 final_document=None
 if s._edit_stack:
  file_dialogs.getSaveFileName=lambda *args:('','');win._on_save_as();final_document=state(s)
 rows.append({'case':case,'phases':phases,'prompt_depths':prompts,'dialog_depths':dialogs,'closed':accepted,'error':bool(errors),'file_exists':path.exists(),'prompt_open':prompt_open,'final_document':final_document})
(out/'original.json').write_text(json.dumps({'rows':rows},indent=2))
# Pending acquisition crosses Save at a stable world point, including a chain
# with one committed segment and a continuation in the root after Save.
pending=[]
for tool,chain in [('line',False),('line',True),('rectangle',False)]:
 s=setup();win._activate_tool(tool)
 def context(p):
  v=V(*p);return ToolContext(vp,v,QPointF(),Qt.NoModifier,SnapResult(v,'none'))
 t=vp.active_tool;t.on_click(context([30,40,50]))
 if chain:t.on_click(context([31,40,50]))
 t.on_hover(context([32,41,50]))
 win._current_path=out/(tool+('-chain' if chain else '-first')+'.original.igz');win._on_save()
 phases=[snapshot('after')]
 anchor=xyz(t.start_point) if t.start_point is not None else None
 t.on_click(context([32,41,50]));phases.append(snapshot('continued'))
 win._on_undo();phases.append(snapshot('undo'));win._on_redo();phases.append(snapshot('redo'))
 pending.append({'case':tool+('-chain' if chain else '-first'),'anchor_after':anchor,'phases':phases})
(out/'original-pending.json').write_text(json.dumps({'rows':pending},indent=2))
QMessageBox.exec=real_exec;win._saved_version=vp.scene.version;win.close()
print(json.dumps({'lifecycle_workflows':len(rows),'pending_workflows':len(pending)}))
