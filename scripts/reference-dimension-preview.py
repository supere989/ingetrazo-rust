#!/usr/bin/env python3
"""Only missing operative Dimension preview observations; original32 unchanged."""
import os,sys,json,math
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from PySide6.QtCore import QPointF
from views.viewport import Viewport
from tools.dimension import DimensionTool
from core.group import Group
from core.mesh import Mesh
from core import axes,units
from formats import igz
app=QApplication([]);out=Path(sys.argv[1]);out.mkdir(exist_ok=True);cases=[];rows=[]
recipes=[('front-z',0,math.pi/2,[1,2,3],[4,6,12],[7,8,20],'m',2),('side-x',0,0,[1,2,3],[9,6,6],[20,8,8],'mm',1),('aligned',-.7,.5,[0,0,0],[3,4,0],[-4,3,0],'m',2),('axial-x',-.7,.5,[0,0,0],[3,4,0],[2,8,0],'m',3),('axial-y',-.7,.5,[0,0,0],[3,4,0],[8,2,0],'ft-in',3),('nested',math.pi/6,0,[0,0,0],[3,4,5],[8,2,9],'m',2)]
def point(v):return list(v.toTuple())
for case,yaw,pitch,a,b,cursor,unit,precision in recipes:
 win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);s=vp.scene
 if case=='nested':
  mesh=Mesh();mesh.add_edge(V(0,0,0),V(1,0,0));child=Group(mesh,'child');child.xform=QMatrix4x4();child.xform.rotate(30,V(0,0,1));root=Group(name='parent');root.children=[child];s.groups=[root]
 units.apply_units(s,dict(length=unit,precision=precision));igz.save_scene(s,out/(case+'-input.igz'))
 if case=='nested':vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0])
 axes.sync(s.drawing_frame);units.bind_scene(s)
 vp.camera.target=V(0,0,0);vp.camera.distance=20;vp.camera.yaw=yaw;vp.camera.pitch=pitch;vp.camera.perspective=False;vp.camera.set_aspect(800,600)
 tool=DimensionTool();vp.active_tool=tool;tool.on_activate(vp)
 cases.append(dict(case=case,yaw=yaw,pitch=pitch,a=a,b=b,cursor=cursor,unit=unit,precision=precision))
 def ctx(p):return SimpleNamespace(viewport=vp,world=V(*p),screen=QPointF(400,300))
 def record(phase):
  label=tool.value_label();rows.append(dict(case=case,phase=phase,label=None if label is None else dict(text=label[0],point=point(label[1])),lines=[[point(a),point(b)]for a,b in tool.rubber_band_lines()],dimensions=[dict(a=point(d.a),b=point(d.b),offset=point(d.offset),axis=d.axis)for d in s.dimensions],undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack)))
 tool.on_hover(ctx(a));record('before');tool.on_click(ctx(a));record('first');tool.on_hover(ctx(b));record('span');tool.on_click(ctx(b));record('second');tool.on_hover(ctx(cursor));record('preview');tool.on_hover(ctx(cursor));record('same');tool.on_cancel(vp);record('cancel')
 tool.on_hover(ctx(a));tool.on_click(ctx(a));tool.on_hover(ctx(b));tool.on_click(ctx(b));tool.on_hover(ctx(cursor));tool.on_click(ctx(cursor));record('commit');vp.history.undo();record('undo');vp.history.redo();record('redo')
 if case=='nested':vp.end_group_edit()
 igz.save_scene(s,out/(case+'-original.igz'))
(out/'preview-original.json').write_text(json.dumps(dict(cases=cases,states=rows),indent=2));print(json.dumps(dict(states=len(rows))))
