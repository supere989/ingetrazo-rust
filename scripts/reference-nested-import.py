#!/usr/bin/env python3
"""Literal original import commands and Viewport contexts; no rendered GPU claim."""
import json,os,sys,zipfile
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.viewport import Viewport
from core.group import Group
from core.mesh import Mesh
from core.layers import Layer
from core.history import SnapshotImport
from formats import igz,obj,dxf_in
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
app=QApplication([])
with zipfile.ZipFile('docs/audits/import-preferences/records.zip') as z:
 for name in ['ambiguous.obj','header.dxf']:(out/name).write_bytes(z.read(name))
(out/'zero.dxf').write_text((out/'header.dxf').read_text().replace('8\nTest\n','8\n0\n'))
(out/'named.obj').write_text('o panel\n'+(out/'ambiguous.obj').read_text())
(out/'parts.obj').write_text('v 0 0 0\nv 1 0 0\nv 0 1 0\nv 2 0 0\nv 3 0 0\nv 2 1 0\no Left\nf 1 2 3\no Right\nf 4 5 6\n')
def seed(s):
 s.mesh.add_edge(V(-1,0,0),V(0,0,0));s.units={'length':'cm','precision':3}
 mesh=Mesh();mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)])
 a=Group(mesh,'Leaf');a.xform=M();a.xform.translate(1,2,0)
 b=Group(mesh,'Sibling');b.xform=M();b.xform.translate(5,0,0)
 outer=Group(name='Outer');outer.children=[a,b];outer.xform=M();outer.xform.translate(8,-3,2);outer.xform.scale(-2,3,1)
 existing=Group(name='Test');existing.mesh.add_edge(V(50,0,0),V(51,0,0));existing.layer='Test';s.layers.append(Layer('Test'))
 s.groups=[outer,existing]
def vec(v):return list(v.toTuple())
def mesh_state(mesh,m=None):
 def point(p):return vec(m.map(p) if m is not None else p)
 def ring(vertices):
  p=[point(v) for v in vertices]
  return min([p[i:]+p[:i] for i in range(len(p))]) if p else []
 return {'points':sorted(point(v.position)for v in mesh.vertices),'edges':sorted([sorted([point(e.a),point(e.b)])for e in mesh.edges]),'faces':sorted([{'outer':ring(f.vertices),'holes':sorted(ring(h)for h in f.holes),'attrs':f.attrs}for f in mesh.faces],key=lambda f:json.dumps(f,sort_keys=True))}
def shape(s):
 def group(g,parent):
  matrix=parent*(g.xform if g.xform is not None else M())
  return {'name':g.name,'layer':g.layer,'mesh':mesh_state(g.mesh,matrix),'children':[group(c,matrix)for c in g.children]}
 return {'active':mesh_state(s.mesh),'loose':mesh_state(s.loose_mesh),'groups':[group(g,M())for g in s.groups],'layers':[l.name for l in s.layers],'units':s.units}
rows=[];cases=[]
for context in ['root','container','leaf']:
 for filename in ['ambiguous.obj','named.obj','parts.obj','header.dxf','zero.dxf']:
  name=context+'-'+Path(filename).stem
  win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp;s=vp.scene;seed(s)
  igz.save_scene(s,out/(name+'-input.igz'))
  if context!='root':vp.begin_group_edit(s.groups[0])
  if context=='leaf':vp.begin_group_edit(s.edit_group.children[0])
  def record(phase):rows.append({'case':name,'phase':phase,'depth':len(s._edit_stack),'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack),'scene':shape(s)})
  record('before');scale=.001 if filename.endswith('.dxf') else .01
  if filename.endswith('.obj'):loader=lambda scene:obj.load_obj(scene,out/filename,scale=scale)
  else:loader=lambda scene:dxf_in.load_dxf(scene,out/filename,scale=scale,name=Path(filename).stem)
  vp.history.execute(SnapshotImport(loader));record('import');vp.history.undo();record('undo');vp.history.redo();record('redo')
  vp.end_group_edit();record('finish');igz.save_scene(s,out/(name+'-original.igz'))
  vp.history.undo();record('finish-undo');vp.history.redo();record('finish-redo')
  cases.append({'name':name,'context':context,'file':filename,'unit':'mm' if filename.endswith('.dxf') else 'cm'})
  win.close()
(out/'original-nested-import.json').write_text(json.dumps({'cases':cases,'states':rows},indent=2)+'\n')
print(json.dumps({'cases':len(cases),'states':len(rows),'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','actual_source':'SnapshotImport, loaders and Viewport context transitions','rendered_gpu_claim':False}))
