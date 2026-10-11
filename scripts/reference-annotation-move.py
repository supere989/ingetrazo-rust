#!/usr/bin/env python3
import sys,os,json
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from PySide6.QtCore import QPointF,Qt
from views.viewport import Viewport
from tools.move import MoveTool
from core.dimension import Dimension
from core.textlabel import TextLabel
from formats import igz
p=Path(sys.argv[1]);app=QApplication([]);v=Viewport();igz.load_into(v.scene,p/'input.igz');s=v.scene
m=QMatrix4x4();m.translate(3,2,0);s.groups[0].children[0].xform=m
s.dimensions=[Dimension(V(20,0,0),V(21,0,0),V(0,1,0)),Dimension(V(-5,2,0),V(5,2,0),V(0,1,0)),Dimension(V(-5,2,0),V(21,0,0),V(0,1,0))]
for d in s.dimensions:d.bind(s)
s.text_labels=[TextLabel(V(20,0,0),V(.1,.2,.3),'first'),TextLabel(V(21,0,0),V(.1,.2,.3),'second')];igz.save_scene(s,p/'move-input.igz')
def point(v):return[v.x(),v.y(),v.z()]
rows=[x for x in json.loads((p/'original.json').read_text())['states'] if x['case']!=sys.argv[2]] if len(sys.argv)>2 else []
for case in ([sys.argv[2]] if len(sys.argv)>2 else ['free','line','partial','riding','mixed','nested','guide','copy']):
 window=QMainWindow();vp=Viewport();window.setCentralWidget(vp);igz.load_into(vp.scene,p/'move-input.igz');s=vp.scene;tool=MoveTool();held=list(s.text_labels);dims=list(s.dimensions)
 if case=='one-riding':
  s.dimensions[1].b=V(-3,-1,0);s.dimensions[1].bind(s);igz.save_scene(s,p/'one-riding-input.igz')
 if case=='nested':vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0])
 if case=='guide':targets=[s.guides[0]]
 else:
  targets=[s.text_labels[0],s.dimensions[{'line':1,'partial':2,'riding':1,'one-riding':1}.get(case,0)]]
  if case in ['mixed','riding','one-riding']:targets.append(s.edges[12])
 s.selection=set(targets)
 def record(phase):rows.append(dict(case=case,phase=phase,dimensions=[dict(a=point(d.a),b=point(d.b),offset=point(d.offset)) for d in s.dimensions],labels=[dict(anchor=point(t.anchor),offset=point(t.offset),text=t.text)for t in s.text_labels],identity=[next(i for i,x in enumerate(held)if x is t)for t in s.text_labels],vertices=[point(x.position)for x in s.mesh.vertices],selection=len(s.selection),undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),active=tool.start_point is not None))
 def ctx(world):return SimpleNamespace(viewport=vp,world=V(*world),screen=QPointF(400,300))
 record('before');tool.on_click(ctx([0,0,0]));record('grab')
 if case=='copy':tool.on_key(vp,Qt.Key_Control,Qt.ControlModifier)
 delta=[1,2,0]if case in ['mixed','riding','one-riding'] else[.7,.8,.9]
 tool.on_hover(ctx(delta));record('preview');tool.on_cancel(vp);record('cancel')
 tool.on_click(ctx([0,0,0]));
 if case=='copy':tool.on_key(vp,Qt.Key_Control,Qt.ControlModifier)
 tool.on_hover(ctx(delta));tool.on_click(ctx(delta));record('commit');
 if case=='parked':vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0]);record('entry')
 vp.history.undo();record('undo');vp.history.redo();record('redo')
 if case in ['nested','parked']:vp.end_group_edit();record('finish');vp.history.undo();record('finish-undo');vp.history.redo();record('finish-redo')
order=['free','line','partial','riding','mixed','nested','guide','copy','parked','one-riding'];rows.sort(key=lambda x:order.index(x['case']))
(p/'original.json').write_text(json.dumps({'states':rows},indent=2));print(json.dumps({'states':len(rows)}))
