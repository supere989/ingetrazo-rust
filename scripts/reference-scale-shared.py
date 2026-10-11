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
rows=[]
for kind in ['baked','shared-one','shared-two']:
 s=Scene();m=s.mesh if kind in ['loose','mixed'] else Mesh()
 face=m.add_face([V(0,0,0),V(2,0,0),V(2,3,0),V(0,3,0)])
 face.attrs={'vendor':71,'texture':{'path':'scan','uvw':[.25,0,0,.1,0,.5,0,.2]}}
 selected=[]
 if kind in ['loose','mixed']:selected.append(face)
 if kind in ['baked','shared-one','shared-two']:
  g=Group(m,name=kind)
  if kind in ['placed','reflected']:g.xform=M(.8,-.6,0,3,.6,.8,0,-2,0,0,-1 if kind=='reflected' else 1,1,0,0,0,1)
  if kind=='baked-rotated':g.axes=M(.8,-.6,0,3,.6,.8,0,-2,0,0,1,1,0,0,0,1)
  s.groups.append(g);selected.append(g)
  if kind.startswith('shared'):
   sibling=Group(m,name='shared sibling');s.groups.append(sibling)
   if kind=='shared-two':selected.append(sibling)
   independent=Mesh();independent.add_face([V(0,0,0),V(2,0,0),V(2,3,0),V(0,3,0)]).attrs=dict(face.attrs);s.groups.append(Group(independent,name='independent equal'))
 if kind in ['image','mixed']:
  im=ImagePlane('scan',V(1,2,0),V(2,0,0),V(0,3,0));s.image_planes.append(im);selected.append(im)
 s.select(selected);h=History(s);vp=types.SimpleNamespace(scene=s,history=h,update=lambda:None,flash_status=lambda text:None)
 t=ScaleTool();t.on_activate(vp);states={'before':snap(s,h)}
 g=next(g for g in reversed(t._grips) if g.kind(t._active_axes())=='corner');grab=t._grip_pos(g);t._grab(vp,g,(0.,0.));states['grab']=snap(s,h)
 hover=grab+(grab-t._anchor)*.75
 ctx=types.SimpleNamespace(viewport=vp,screen=QPointF(20.,20.),snap=types.SimpleNamespace(kind='endpoint',point=hover))
 t.on_hover(ctx);states['preview']=snap(s,h)
 t.on_key(vp,Qt.Key_Control,None);states['center']=snap(s,h)
 accepted=t.on_value(vp,2.);states['commit']=snap(s,h)
 accepted_retype=t.on_value(vp,-1.);states['retype']=snap(s,h)
 h.undo();states['undo']=snap(s,h);h.redo();states['redo']=snap(s,h)
 t._box_version=-1;t._refresh_box(vp);g=next(g for g in reversed(t._grips) if g.kind(t._active_axes())=='corner');cancel_grab=t._grip_pos(g);t._grab(vp,g,(0.,0.));cancel_hover=cancel_grab+(cancel_grab-t._anchor)*.5
 ctx.snap.point=cancel_hover;t.on_hover(ctx);states['cancel-preview']=snap(s,h);t.on_cancel(vp);states['cancel']=snap(s,h)
 rows.append({'kind':kind,'grab':vec(grab),'hover':vec(hover),'cancel_grab':vec(cancel_grab),'cancel_hover':vec(cancel_hover),'accepted':[accepted,accepted_retype],'states':states,'history_error':h.last_error})
Path(sys.argv[1]).write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','cases':rows},indent=2)+'\n')
print(len(rows),'actual Scale tool/history workflows captured')
