#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from PySide6.QtCore import QPointF,Qt
from core.mesh import Mesh
from tools.fillet import FilletTool
from tools.base import ToolContext
from views.viewport import Viewport
app=QApplication([])
source=json.loads(Path(sys.argv[1]).read_text());rows=[]
def snap(m):
 vs=list(m.vertices);index={id(v):i for i,v in enumerate(vs)}
 return dict(vertices=[[v.position.x(),v.position.y(),v.position.z()]for v in vs],edges=[dict(ends=[index[id(e.v0)],index[id(e.v1)]],soft=e.soft,curve=e.curve,layer=e.layer,hidden=e.hidden,attrs={})for e in m.edges],faces=[dict(loop=[index[id(v)]for v in f.loop],holes=[[index[id(v)]for v in h]for h in f.hole_loops],attrs=f.attrs,interior=f.interior)for f in m.faces])
for recipe in ['endpoint-free-wire','open-neighbor','nested-translated']:
 row=next(x for x in source['cases'] if x['name']==('endpoint-free-wire' if recipe=='nested-translated' else recipe))
 vp=Viewport(None);vp.flash_status=lambda *a,**k:None;m=Mesh();before=copy.deepcopy(row['before'])
 if recipe=='nested-translated':
  before['vertices']=before['vertices'][:8];before['edges']=before['edges'][:12];before['faces']=before['faces'][:6]
  for e in before['edges']:e.update(soft=False,curve=None,layer=None,hidden=False)
  for f in before['faces']:f.update(attrs={},interior=False)
 vs=[m.vertex(V(*p))for p in before['vertices']]
 for e in before['edges']:
  q=m.add_edge(vs[e['ends'][0]].position,vs[e['ends'][1]].position);q.soft=e['soft'];q.curve=e['curve'];q.layer=e['layer'];q.hidden=e['hidden']
 for f in before['faces']:
  q=m.add_face([vs[i].position for i in f['loop']],[[vs[i].position for i in h]for h in f['holes']]);q.attrs=f['attrs'];q.interior=f['interior']
 vp.scene.mesh=m
 if recipe=='nested-translated':
  from core.group import Group
  from PySide6.QtGui import QMatrix4x4
  child=Group(m,'child');child.xform=QMatrix4x4();child.xform.translate(1,2,0)
  outer=Group(Mesh(),'outer');outer.xform=QMatrix4x4();outer.xform.translate(4,5,0);outer.children=[child];vp.scene.mesh=Mesh();vp.scene.groups=[outer];vp.begin_group_edit(outer);vp.begin_group_edit(child);m=vp.scene.mesh
 edges=[m.edges[i]for i in row['selected']];vp.scene.select(edges);FilletTool.radius=.1;FilletTool.segments=8;t=FilletTool();vp.set_active_tool(t);states={}
 def state():return dict(mesh=snap(m),undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),selection=len(vp.scene.selection),preview_faces=len(t.preview_faces()),preview_lines=len(t.rubber_band_lines()))
 def ctx(p):return ToolContext(viewport=vp,world=p,screen=QPointF(),modifiers=Qt.NoModifier,snap=None)
 states['activate']=state();t.on_segments_value(vp,6);states['segments']=state();p=(edges[0].a+edges[0].b)*.5;t.on_click(ctx(p));t.on_hover(ctx(p+V(0,.25,0)));states['sizing']=state();t.on_value(vp,5);states['refusal']=state();t.on_cancel(vp);states['cancel']=state();vp.scene.select(edges);t=FilletTool();vp.set_active_tool(t);t.on_value(vp,.2);states['commit']=state();vp.history.undo();states['undo']=state();vp.history.redo();states['redo']=state()
 if recipe=='nested-translated':
  vp.end_one_group_edit();m=child.mesh;states['finish-child']=state();vp.end_one_group_edit();m=outer.children[0].mesh;states['finish-outer']=state();vp.history.undo();m=outer.children[0].mesh;states['finish-undo']=state();vp.history.redo();m=outer.children[0].mesh;states['finish-redo']=state()
 rows.append(dict(name=recipe,states=states))
Path(sys.argv[2]).write_text(json.dumps(dict(source_pin=source['source_pin'],apps=rows),indent=2))
