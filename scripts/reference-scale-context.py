#!/usr/bin/env python3
import json,sys,types
from pathlib import Path
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from PySide6.QtCore import QPointF
from views.viewport import Viewport
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.history import History
from core.image_plane import ImagePlane
from tools.scale import ScaleTool
app=QApplication([])
def vec(p):return [p.x(),p.y(),p.z()]
def geometry(m):return [vec(v.position)for v in m.vertices]
def group(g):return {'name':g.name,'vertices':geometry(g.mesh),'transform':list(g.xform.data())if g.xform is not None else None,'children':[group(c)for c in g.children]}
def state(v,t):return {'mesh':geometry(v.scene.mesh),'groups':[group(g)for g in v.scene.groups],'images':[[vec(im.origin),vec(im.u),vec(im.v)]for im in v.scene.image_planes],'depth':len(v.scene._edit_stack),'active':t._grip is not None,'undo':len(v.history.undo_stack),'redo':len(v.history.redo_stack),'error':v.history.last_error}
rows=[]
for kind in ['nested-preview','nested-commit','image-preview','mixed-preview','entry-preview']:
 window=QMainWindow();v=Viewport(window);window.setCentralWidget(v);v.scene=Scene();v.history=History(v.scene)
 m=Mesh();m.add_face([V(0,0,0),V(2,0,0),V(2,3,0),V(0,3,0)])
 child=Group(m,name='child');child.xform=M();child.xform.translate(1,2,0)
 outer=Group(Mesh(),name='outer');outer.children=[child];outer.xform=M();outer.xform.translate(4,5,0);v.scene.groups=[outer]
 t=ScaleTool();v.active_tool=t
 if kind!='entry-preview':v.begin_group_edit(outer);v.begin_group_edit(child)
 im=ImagePlane('scan',V(1,2,0),V(2,0,0),V(0,3,0));v.scene.image_planes.append(im)
 if kind=='entry-preview':v.scene.select([outer])
 elif kind=='image-preview':v.scene.select([im])
 elif kind=='mixed-preview':v.scene.select([v.scene.mesh.faces[0],im])
 else:v.scene.select([v.scene.mesh.faces[0]])
 t.on_activate(v);states={'before':state(v,t)}
 grip=next(g for g in reversed(t._grips)if g.kind(t._active_axes())=='corner');p=t._grip_pos(grip);t._grab(v,grip,(0,0));hover=p+(p-t._anchor)
 t.on_hover(types.SimpleNamespace(viewport=v,screen=QPointF(20,20),snap=types.SimpleNamespace(kind='endpoint',point=hover)));states['preview']=state(v,t)
 if kind=='nested-commit':t.on_value(v,2.);states['commit']=state(v,t)
 if kind=='entry-preview':v.begin_group_edit(outer)
 else:v.end_one_group_edit()
 states['boundary']=state(v,t)
 if kind=='nested-commit':t.on_value(v,3.);states['retype']=state(v,t);v.history.undo();states['undo']=state(v,t);v.history.redo();states['redo']=state(v,t)
 else:t.on_cancel(v);states['tool-cancel']=state(v,t)
 rows.append({'name':kind,'states':states,'grab':vec(p),'hover':vec(hover)})
 v.deleteLater()
Path(sys.argv[1]).write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','cases':rows},indent=2)+'\n')
