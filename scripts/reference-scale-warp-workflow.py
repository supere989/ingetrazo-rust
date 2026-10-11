#!/usr/bin/env python3
"""Frozen actual Scale tool/History capture; no reimplementation of transforms."""
import json,sys,types
from pathlib import Path
from PySide6.QtCore import QPointF,Qt
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.image_plane import ImagePlane
from core.history import History
from tools.scale import ScaleTool

def vec(p):return [p.x(),p.y(),p.z()]
def matrix(m):return list(m.data()) if m is not None else None
def mesh(m):return {'vertices':[vec(v.position) for v in m.vertices], 'faces':[{'vertices':[vec(v)for v in f.vertices],'attrs':dict(f.attrs)}for f in m.faces]}
def snap(s,h):return {'mesh':mesh(s.mesh),'groups':[{'mesh':mesh(g.mesh),'transform':matrix(g.xform),'axes':matrix(g.axes)}for g in s.groups], 'images':[[vec(im.origin),vec(im.u),vec(im.v)]for im in s.image_planes], 'undo':len(h.undo_stack),'redo':len(h.redo_stack)}
s=Scene();face=s.mesh.add_face([V(0,0,0),V(1,0,0),V(1,1,0),V(0,1,0)]);face.attrs={'vendor':71}
edge=s.mesh.add_edge(V(0,0,0),V(2,0,1));s.select([edge]);h=History(s);vp=types.SimpleNamespace(scene=s,history=h,update=lambda:None,flash_status=lambda text:None)
t=ScaleTool();t.on_activate(vp);g=t._grips[0];grab=t._grip_pos(g);t._grab(vp,g,(0.,0.));states={'before':snap(s,h)}
a=t.on_value(vp,2.);states['commit']=snap(s,h);b=t.on_value(vp,3.);states['retype']=snap(s,h)
h.undo();states['undo']=snap(s,h);h.redo();states['redo']=snap(s,h)
Path(sys.argv[1]).write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','grab':vec(grab),'accepted':[a,b],'states':states,'history_error':h.last_error},indent=2)+'\n')
print('One actual partial Scale/autofold/history workflow captured')
