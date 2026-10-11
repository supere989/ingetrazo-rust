#!/usr/bin/env python3
"""Missing Scale modifier/context laws through pinned operative Qt handlers."""
import json,sys,types
from pathlib import Path
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M,QKeyEvent
from PySide6.QtCore import QPointF,Qt,QEvent
from views.viewport import Viewport
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.history import History
from core.image_plane import ImagePlane
from tools.scale import ScaleTool
app=QApplication([])
def vec(p):return [p.x(),p.y(),p.z()]
def state(v,t):return {'positions':[vec(x.position)for x in v.scene.mesh.vertices],'images':[[vec(x.origin),vec(x.u),vec(x.v)]for x in v.scene.image_planes],'depth':len(v.scene._edit_stack),'active':t._grip is not None,'undo':len(v.history.undo_stack),'redo':len(v.history.redo_stack),'about_center':t.about_center,'uniform':t.uniform}
def setup(context=False):
 w=QMainWindow();v=Viewport(w);w.setCentralWidget(v);v.resize(1000,700);v.scene=Scene();v.history=History(v.scene);m=Mesh();m.add_face([V(0,0,0),V(2,0,0),V(2,3,0),V(0,3,0)])
 if context:
  child=Group(m,name='child');child.xform=M();child.xform.translate(1,2,0);outer=Group(Mesh(),name='outer');outer.children=[child];outer.xform=M();outer.xform.translate(4,5,0);v.scene.groups=[outer];v.begin_group_edit(outer);v.begin_group_edit(child);image=ImagePlane('scan',V(1,2,0),V(2,0,0),V(0,3,0));v.scene.image_planes.append(image);v.scene.select([image])
 else:v.scene.mesh=m;v.scene.select([m.faces[0]])
 t=ScaleTool();v.active_tool=t;t.on_activate(v);return w,v,t
rows=[]
for name,keys in [('ctrl-tap',['ctrl']),('ctrl-double',['ctrl','ctrl']),('shift-tap',['shift']),('shift-double',['shift','shift']),('ctrl-shift',['ctrl','shift']),('ctrl-shift-twice',['ctrl','shift','ctrl','shift'])]:
 w,v,t=setup();g=next(g for g in reversed(t._grips)if tuple(g.mask)==(0,));grab=t._grip_pos(g);t._grab(v,g,(0,0));hover=grab+(grab-t._anchor)*.5;t.on_hover(types.SimpleNamespace(viewport=v,screen=QPointF(20,20),snap=types.SimpleNamespace(kind='endpoint',point=hover)));states={'before-taps':state(v,t)}
 for key in keys:
  q=Qt.Key_Control if key=='ctrl'else Qt.Key_Shift;mod=Qt.ControlModifier if key=='ctrl'else Qt.ShiftModifier;app.sendEvent(v,QKeyEvent(QEvent.KeyPress,q,mod));app.sendEvent(v,QKeyEvent(QEvent.KeyRelease,q,Qt.NoModifier))
 states['after-taps']=state(v,t);accepted=t.on_value(v,2.);states['commit']=state(v,t);v.history.undo();states['undo']=state(v,t);v.history.redo();states['redo']=state(v,t);rows.append({'name':name,'kind':'rapid','keys':keys,'grab':vec(grab),'hover':vec(hover),'accepted':accepted,'states':states});v.deleteLater()
for name in ['image-finish-preview-commit','image-exit-two','image-commit-finish-retype']:
 w,v,t=setup(True);g=next(g for g in reversed(t._grips)if g.kind(t._active_axes())=='corner');grab=t._grip_pos(g);t._grab(v,g,(0,0));hover=grab+(grab-t._anchor);t.on_hover(types.SimpleNamespace(viewport=v,screen=QPointF(20,20),snap=types.SimpleNamespace(kind='endpoint',point=hover)));states={'preview':state(v,t)};accepted=[]
 if name=='image-commit-finish-retype':accepted.append(t.on_value(v,2.));states['commit-before']=state(v,t)
 v.end_one_group_edit();states['boundary']=state(v,t)
 if name=='image-exit-two':v.end_one_group_edit();states['outer-boundary']=state(v,t)
 if name!='image-commit-finish-retype':accepted.append(t.on_value(v,2.));states['commit']=state(v,t)
 accepted.append(t.on_value(v,3.));states['retype']=state(v,t);v.history.undo();states['undo']=state(v,t);v.history.redo();states['redo']=state(v,t);t.on_cancel(v);states['tool-cancel']=state(v,t);rows.append({'name':name,'kind':'image','grab':vec(grab),'hover':vec(hover),'accepted':accepted,'states':states});v.deleteLater()
Path(sys.argv[1]).write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','cases':rows,'scope':'actual Qt Viewport modifier key delivery; grab/hover/value/context handler recipes, no OS capture'},indent=2)+'\n')
