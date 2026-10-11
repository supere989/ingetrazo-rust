#!/usr/bin/env python3
"""Only missing Select hover Delete key controls; no old Delete recapture."""
import os,sys,json,math
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QKeyEvent
from PySide6.QtCore import QPointF,Qt,QEvent
from views.viewport import Viewport
from tools.select import SelectTool
from formats import igz
app=QApplication([]);p=Path(sys.argv[1]);old=json.loads((p/'original-hover.json').read_text());rows=[]
def point(v):return [v.x(),v.y(),v.z()]
def dim(d):return dict(a=point(d.a),b=point(d.b),offset=point(d.offset),text=d.text or '',axis=d.axis,layer=d.layer)
for case in ['hover-label-root','hover-dimension-root','hover-label-nested','hover-dimension-nested','selection-priority']:
 w=QMainWindow();vp=Viewport();w.setCentralWidget(vp);vp.resize(800,600);igz.load_into(vp.scene,p/'hover-input.igz');s=vp.scene
 held={k:list(getattr(s,name))for k,name in [('guides','guides'),('dimensions','dimensions'),('labels','text_labels')]}
 if case.endswith('nested'):vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0])
 s.clear_selection();vp.camera.target=V(0,0,0);vp.camera.distance=10;vp.camera.yaw=0;vp.camera.pitch=math.pi/2;vp.camera.perspective=False;vp.camera.set_aspect(800,600)
 lookup='dimension'if 'dimension' in case else 'glyph';screen=next(r['screen']for r in old['states']if r['case']==lookup)
 tool=SelectTool();vp.active_tool=tool;tool.on_activate(vp);tool.on_hover(SimpleNamespace(viewport=vp,screen=QPointF(*screen)))
 assert vp._hover_entity is (s.dimensions[0]if lookup=='dimension'else s.text_labels[0])
 if case=='selection-priority':s.select([s.dimensions[0]])
 def record(phase):
  rows.append(dict(case=case,phase=phase,screen=screen,guides=[g.to_dict()for g in s.guides],dimensions=[dim(d)for d in s.dimensions],labels=[t.to_dict()for t in s.text_labels],identity={k:[next((i for i,v in enumerate(held[k])if v is x),-1)for x in items]for k,items in [('guides',s.guides),('dimensions',s.dimensions),('labels',s.text_labels)]},selection=len(s.selection),undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),groups=len(s.edit_group.children if s.edit_group is not None else s.groups),faces=len(s.faces)))
 record('before');vp.keyPressEvent(QKeyEvent(QEvent.KeyPress,Qt.Key_Delete,Qt.NoModifier));record('delete');vp.history.undo();record('undo');vp.history.redo();record('redo')
 if case.endswith('nested'):vp.end_group_edit();record('finish');vp.history.undo();record('finish-undo');vp.history.redo();record('finish-redo')
(p/'hover-delete-original.json').write_text(json.dumps(dict(states=rows),indent=2))
print(json.dumps(dict(states=len(rows))))
