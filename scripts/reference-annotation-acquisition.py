#!/usr/bin/env python3
import os,sys,json,math
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V
from PySide6.QtCore import QPointF
from views.viewport import Viewport
from tools.move import MoveTool
from core.dimension import Dimension
from core.textlabel import TextLabel
from core.layers import Layer
from core.group import Group
from core.mesh import Mesh
from formats import igz
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True);app=QApplication([]);v=Viewport();s=v.scene
s.mesh.add_face([V(-2,-2,0),V(2,-2,0),V(2,2,0),V(-2,2,0)])
s.dimensions=[Dimension(V(-1,0,0),V(1,0,0),V(0,1,0))];s.dimensions[0].bind(s)
s.text_labels=[TextLabel(V(0,0,0),V(0,1,0),'first\nsecond',layer='named')];s.layers.append(Layer('named'))
m=Mesh();m.add_edge(V(5,0,0),V(6,0,0));child=Group(m,'child');parent=Group(Mesh(),'parent');parent.children=[child];s.groups=[parent]

if len(sys.argv)==2:igz.save_scene(s,out/'input.igz')
rows=json.loads((out/'original.json').read_text())['states'] if len(sys.argv)>2 else []
for case in ([sys.argv[2]] if len(sys.argv)>2 else ['glyph','dimension','leader-miss','hidden','locked','nested','riding-edge']):
 win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);vp.resize(800,600);igz.load_into(vp.scene,out/'input.igz');s=vp.scene
 if case=='riding-edge':
  s.dimensions[0].a=V(-2,-2,0);s.dimensions[0].b=V(2,-2,0);s.dimensions[0].offset=V(0,0,0);s.dimensions[0].bind(s);igz.save_scene(s,out/'riding-input.igz')
 if case=='nested':vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0])
 vp.camera.target=V(0,0,0);vp.camera.distance=10;vp.camera.yaw=0;vp.camera.pitch=math.pi/2;vp.camera.perspective=False;vp.camera.set_aspect(800,600)
 layer=next(x for x in s.layers if x.name=='named');layer.visible=case!='hidden';layer.locked=case=='locked'
 pp=vp._world_to_pixel(s.text_labels[0].position());p=[pp[0]+8,pp[1]-8]
 if case=='dimension':p=[400,pp[1]]
 if case=='riding-edge':p=list(vp._world_to_pixel(V(0,-2,0)))
 if case=='leader-miss':p=[400,(pp[1]+300)/2]
 tool=MoveTool();s.selection=set()
 hitlab=vp.pick_text_label(*p,rect_only=True);hitdim=vp.pick_dimension(*p)
 def record(phase):
  row=dict(case=case,phase=phase,screen=p,hit_label=next((i for i,x in enumerate(s.text_labels)if x is hitlab),None),hit_dimension=next((i for i,x in enumerate(s.dimensions)if x is hitdim),None),labels=[list(t.offset.toTuple())for t in s.text_labels],dimensions=[dict(a=list(d.a.toTuple()),b=list(d.b.toTuple()),offset=list(d.offset.toTuple()))for d in s.dimensions],selection=len(s.selection),undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),active=tool.start_point is not None)
  if case=='riding-edge':row['vertices']=[list(x.position.toTuple())for x in s.mesh.vertices]
  rows.append(row)
 def ctx(w):return SimpleNamespace(viewport=vp,screen=QPointF(*p),world=V(*w))
 record('before');tool.on_click(ctx([0,0,0]));record('grab');tool.on_hover(ctx([.5,.25,0]));record('preview');tool.on_cancel(vp);record('cancel');tool.on_click(ctx([0,0,0]));tool.on_hover(ctx([.5,.25,0]));tool.on_click(ctx([.5,.25,0]));record('commit');vp.history.undo();record('undo');vp.history.redo();record('redo')
(out/'original.json').write_text(json.dumps({'states':rows},indent=2))
