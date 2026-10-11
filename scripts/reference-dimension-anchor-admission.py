#!/usr/bin/env python3
"""Missing source local registry admission; operative Dimension/Move commands."""
import os,sys,json
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from PySide6.QtCore import QPointF
from views.viewport import Viewport
from tools.dimension import DimensionTool
from tools.move import MoveTool
from core.mesh import Mesh
from core.group import Group,iter_placements
from formats import igz
app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(exist_ok=True);rows=[];cases=[]
def point(p):return list(p.toTuple())
def wire():m=Mesh();m.add_edge(V(0,0,0),V(1,0,0));return m
for case in ['near','outside','nested-shared','billboard','orphan']:
 w=QMainWindow();vp=Viewport();w.setCentralWidget(vp);s=vp.scene
 # Offscreen controller proof omits only the GL-owned group preview uploader.
 vp.begin_groups_preview=lambda *args:None
 vp.end_groups_preview=lambda *args:None
 if case in ['near','outside']:s.mesh=wire();m=None
 else:
  mesh=wire() if case!='orphan' else Mesh()
  if case=='orphan':mesh.vertex(V(0,0,0));mesh.vertex(V(1,0,0));mesh.add_edge(V(5,0,0),V(6,0,0))
  leaf=Group(mesh,'leaf');root=leaf;m=None
  if case=='nested-shared':
   leaf.xform=QMatrix4x4();leaf.xform.translate(1,2,0)
   sibling=Group(mesh,'shared');sibling.xform=QMatrix4x4();sibling.xform.translate(10,2,0)
   root=Group(name='root');root.children=[leaf,sibling];root.xform=QMatrix4x4();root.xform.translate(10000,20000,30000);root.xform.scale(2,3,1);m=root.xform*leaf.xform
  if case=='billboard':root.billboard=True
  s.groups=[root]
 delta=2e-5 if case!='outside' else 2e-4
 a=V(delta,0,0);b=V(1+delta,0,0)
 if m is not None:a=m.map(a);b=m.map(b)
 cursor=(a+b)*.5+V(0,1,0)
 clicks=[point(a),point(b),point(cursor)]
 igz.save_scene(s,out/(case+'-input.igz'))
 cases.append(dict(case=case,clicks=clicks))
 def ctx(p):return SimpleNamespace(viewport=vp,world=V(*p),screen=QPointF(400,300))
 def record(phase):
  rows.append(dict(case=case,phase=phase,dimensions=[dict(a=point(d.a),b=point(d.b),offset=point(d.offset),axis=d.axis,anchored=[d.anchor_a is not None,d.anchor_b is not None])for d in s.dimensions],selection=len(s.selection),undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack)))
 record('before');tool=DimensionTool();vp.active_tool=tool
 for p in clicks:tool.on_click(ctx(p))
 record('create');vp.history.undo();record('create-undo');vp.history.redo();record('create-redo')
 s.select([s.edges[0]if case in ['near','outside']else s.groups[0]])
 tool=MoveTool();vp.active_tool=tool;tool.on_click(ctx([0,0,0]));tool.on_hover(ctx([1,2,0]));tool.on_click(ctx([1,2,0]));record('move');vp.history.undo();record('undo');vp.history.redo();record('redo');igz.save_scene(s,out/(case+'-original.igz'))
(out/'admission-original.json').write_text(json.dumps(dict(cases=cases,states=rows),indent=2));print(json.dumps(dict(states=len(rows))))
