#!/usr/bin/env python3
import sys,os,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V
from views.viewport import Viewport
from tools.select import erase_entities
from core.guide import Guide
from core.dimension import Dimension
from core.textlabel import TextLabel
from formats import igz
p=Path(sys.argv[1]);app=QApplication([])
vp=Viewport();igz.load_into(vp.scene,p/'input.igz');s=vp.scene
s.guides.extend([Guide(V(10,0,0)),Guide(V(11,0,0))]);s.dimensions.extend([Dimension(V(10,0,0),V(11,0,0),V(0,1,0),'second')]);s.text_labels.extend([TextLabel(V(10,0,0),V(1,0,0),'same'),TextLabel(V(10,0,0),V(1,0,0),'same')]);igz.save_scene(s,p/'delete-input.igz')
rows=[]
def point(v):return [v.x(),v.y(),v.z()]
def dim(d):return dict(a=point(d.a),b=point(d.b),offset=point(d.offset),text=d.text or '',axis=d.axis,layer=d.layer)
def run(case):
 window=QMainWindow();vp=Viewport();window.setCentralWidget(vp);igz.load_into(vp.scene,p/'delete-input.igz');s=vp.scene;held={'guides':list(s.guides),'dimensions':list(s.dimensions),'labels':list(s.text_labels)}
 def record(phase):
  rows.append(dict(case=case,phase=phase,guides=[g.to_dict() for g in s.guides],dimensions=[dim(d) for d in s.dimensions],labels=[t.to_dict() for t in s.text_labels],identity={k:[next((i for i,v in enumerate(held[k]) if v is x),-1) for x in items] for k,items in [('guides',s.guides),('dimensions',s.dimensions),('labels',s.text_labels)]},selection=len(s.selection),undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),groups=len(s.edit_group.children if s.edit_group is not None else s.groups),faces=len(s.faces)))
 if case.startswith('nested-'):vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0])
 record('before')
 targets=[s.guides[0],s.guides[2],s.dimensions[0],s.text_labels[1]]
 if case=='root':targets.append(s.groups[0])
 if case=='nested-mixed':targets.append(s.faces[0])
 s.selection=set(targets);assert erase_entities(vp,targets);record('delete')
 if case=='parked':vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0]);record('entry')
 vp.history.undo();record('undo');vp.history.redo();record('redo')
 if case!='root':vp.end_group_edit();record('finish');vp.history.undo();record('finish-undo');vp.history.redo();record('finish-redo')
 igz.save_scene(s,p/(case+'-original.igz'));record('save')
if len(sys.argv)>2:
 previous=json.loads((p/'original.json').read_text())['states'];run(sys.argv[2]);rows=previous+rows
else:
 for case in ['root','nested-annotations','nested-mixed']:run(case)
(p/'original.json').write_text(json.dumps({'states':rows},indent=2));print(json.dumps({'states':len(rows)}))
