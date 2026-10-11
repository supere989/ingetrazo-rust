#!/usr/bin/env python3
"""Correct source-authoring registry control and actual oblique Fillet handlers."""
import json,sys,hashlib
from pathlib import Path
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from PySide6.QtCore import QPointF,Qt
from core.mesh import _key
from core.fillet import plan_fillet,apply_fillet
from tools.fillet import FilletTool
from tools.base import ToolContext
from views.viewport import Viewport
from tests.test_fillet import box,edge_at
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
def make(name):
 m=box()
 if name=='oblique-cap':
  for v in list(m.vertices):
   if v.position.x()==2:m.place_vertex(v,V(2.5 if v.position.y()==0 else 2,v.position.y(),v.position.z()))
 else:
  matrix=QMatrix4x4();matrix.translate(3,-5,7);matrix.rotate(90,0,1,0);matrix.scale(2,3,.5)
  for v in list(m.vertices):m.place_vertex(v,matrix.map(v.position))
 assert all(m.vertex_at(v.position)is v for v in m.vertices)
 return m

def snap(m,old=None):
 old=old or ([],[],[]);vs=list(m.vertices);index={id(v):i for i,v in enumerate(vs)}
 return dict(vertices=[[v.position.x(),v.position.y(),v.position.z()]for v in vs],vertex_origins=[next((i for i,q in enumerate(old[0])if q is v),None)for v in vs],edges=[dict(ends=[index[id(e.v0)],index[id(e.v1)]],soft=e.soft,curve=e.curve,layer=e.layer,hidden=e.hidden,attrs={},origin=next((i for i,q in enumerate(old[1])if q is e),None))for e in m.edges],faces=[dict(loop=[index[id(v)]for v in f.loop],holes=[[index[id(v)]for v in h]for h in f.hole_loops],attrs=f.attrs,interior=f.interior,origin=next((i for i,q in enumerate(old[2])if q is f),None))for f in m.faces])
app=QApplication([]);rows=[]
for name,radius,segments in [('oblique-cap',.2,8),('turned-squashed',.1,4)]:
 m=make(name);old=(list(m.vertices),list(m.edges),list(m.faces));edges=[edge_at(m,(0,0,1),(2.5,0,1))]if name=='oblique-cap'else list(m.edges);before=snap(m,old);selected=[m.edges.index(e)for e in edges];plan=plan_fillet(m,edges,radius,segments);assert not isinstance(plan,str);apply_fillet(m,plan)
 from core.orient import is_closed
 assert is_closed(m)
 model=dict(name=name,radius=radius,segments=segments,selected=selected,before=before,after=snap(m,old))
 vp=Viewport(None);vp.flash_status=lambda *a,**k:None;vp.scene.mesh=make(name);m=vp.scene.mesh;old=(list(m.vertices),list(m.edges),list(m.faces));edges=[edge_at(m,(0,0,1),(2.5,0,1))]if name=='oblique-cap'else list(m.edges);vp.scene.select(edges);FilletTool.radius=.1;FilletTool.segments=8;t=FilletTool();vp.set_active_tool(t);t.on_segments_value(vp,segments);states={}
 def state():return dict(mesh=snap(m,old),undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),selection=len(vp.scene.selection),preview_faces=len(t.preview_faces()),preview_lines=len(t.rubber_band_lines()))
 def ctx(p):return ToolContext(viewport=vp,world=p,screen=QPointF(),modifiers=Qt.NoModifier,snap=None)
 states['activate']=state();p=(edges[0].a+edges[0].b)*.5;t.on_click(ctx(p));t.on_hover(ctx(p+V(0,.2,0)));states['sizing']=state();t.on_value(vp,radius);states['commit']=state();vp.history.undo();states['undo']=state();vp.history.redo();states['redo']=state();assert is_closed(m)
 rows.append(dict(name=name,model=model,states=states))
Path(sys.argv[1]).write_text(json.dumps(dict(source_pin=PIN,authoring='Mesh.place_vertex maintains actual _registry, unlike obsolete _verts fixture assignment',cases=rows,source_files={p:hashlib.sha256(Path(p).read_bytes()).hexdigest()for p in ['core/mesh.py','core/fillet.py','tools/fillet.py','tests/test_fillet.py']}),indent=2)+'\n')
