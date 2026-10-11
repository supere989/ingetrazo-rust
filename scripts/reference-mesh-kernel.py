#!/usr/bin/env python3
"""Pinned direct Mesh/kernel observations; no source modifications."""
import sys,json,importlib.util
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ingetrazo'))
from PySide6.QtGui import QVector3D as V
from core.mesh import Mesh

def vec(p): return list(V(*p).toTuple())
def fingerprint(m):
 vi={id(v):i for i,v in enumerate(m.vertices)};fi={id(f):i for i,f in enumerate(m.faces)}
 return dict(vertices=[list(v.position.toTuple()) for v in m.vertices],edges=[dict(v=[vi[id(e.v0)],vi[id(e.v1)]],faces=[fi[id(f)] for f in e.faces],soft=e.soft,hidden=e.hidden,curve=e.curve,layer=e.layer) for e in m.edges],faces=[dict(loop=[vi[id(v)] for v in f.loop],holes=[[vi[id(v)] for v in h] for h in f.hole_loops],attrs=f.attrs) for f in m.faces])
def run(c):
 m=Mesh();states=[]
 for op in c['operations']:
  k=op['kind']
  if k=='vertex': m.vertex(V(*op['p']))
  elif k=='face':
   f=m.add_face([V(*p) for p in op['outer']],[[V(*p) for p in h] for h in op.get('holes',[]) ] or None); f.attrs.update(op.get('attrs',{}))
  elif k=='edge':
   e=m.add_edge(V(*op['a']),V(*op['b']));
   if e:
    for key,value in op.get('flags',{}).items():setattr(e,key,value)
  elif k=='place': m.place_vertex(m.vertices[op['vertex']],V(*op['p']))
  elif k=='split': m.split_edge(m.find_edge(m.vertex_at(V(*op['a'])),m.vertex_at(V(*op['b']))),V(*op['p']))
  states.append(fingerprint(m))
 return states
cases=[]
def add(name,ops):
 for op in ops:
  for k in ('p','a','b'):
   if k in op:op[k]=vec(op[k])
  if 'outer'in op:op['outer']=list(map(vec,op['outer']));op['holes']=[list(map(vec,h)) for h in op.get('holes',[])]
 cases.append(dict(name=name,operations=ops))
for name,pts in [('cross-positive',[(.000049,0,0),(.000051,0,0)]),('cross-negative',[(-.000049,0,0),(-.000051,0,0)]),('distance-reject',[(.000049,0,0),(.0001491,0,0)]),('tie-and-order',[(.00005,0,0),(.00015,0,0),(-.00005,0,0),(-.00015,0,0)]),('neighbor-first',[(.000049,.000049,0),(-.000049,-.000049,0),(0,0,0)])]:add(name,[dict(kind='vertex',p=p) for p in pts])
for count in (2,3):
 ops=[dict(kind='face',outer=[(0,0,0),(0,0,2),(x,y,2),(x,y,0)],attrs={'vendor':{'wall':i}}) for i,(x,y) in enumerate([(1,0),(0,1),(-1,0)][:count])]
 ops +=[dict(kind='edge',a=(0,0,0),b=(0,0,2),flags={'soft':True,'hidden':True,'curve':7,'layer':'named'}),dict(kind='split',a=(0,0,0),b=(0,0,2),p=(0,0,1)),dict(kind='split',a=(0,0,0),b=(0,0,1),p=(0,0,0))]
 add('radial-'+str(count),ops)
add('hole-split-weld', [dict(kind='face',outer=[(-3,-3,0),(3,-3,0),(3,3,0),(-3,3,0)],holes=[[(-1,-1,0),(1,-1,0),(1,1,0),(-1,1,0)]],attrs={'mat':'paint','vendor':42}),dict(kind='edge',a=(-1,-1,0),b=(0,-1,0),flags={'soft':True,'curve':9,'layer':'keep'}),dict(kind='split',a=(-1,-1,0),b=(1,-1,0),p=(0,-1,0))])
spec=importlib.util.spec_from_file_location('mesh_bulk',Path(__file__).resolve().parents[1]/'ingetrazo/tests/test_mesh_bulk.py');bulk=importlib.util.module_from_spec(spec);spec.loader.exec_module(bulk)
for seed in (range(8) if len(sys.argv)<3 else []):
 faces=bulk._gen_faces(120,seed)
 add('bulk-seed-'+str(seed),[dict(kind='face',outer=o,holes=h,attrs=a or {}) for o,h,a in faces])
 # Original bulk law is independently checked against its sequential path.
 seq=bulk._fingerprint(bulk._build_sequential(faces));blk=bulk._fingerprint(bulk._build_bulk(faces));assert sorted(seq.pop('verts'))==sorted(blk.pop('verts'));assert seq==blk
# One representative larger import-like grid, retaining the same source recipe law.
if len(sys.argv)<3:
 faces=bulk._gen_faces(1200,23)
 add('large-grid-1200',[dict(kind='face',outer=o,holes=h,attrs=a or {}) for o,h,a in faces])
add('exact-half-diagonal',[dict(kind='vertex',p=(.03125,.03125,.03125)),dict(kind='vertex',p=(.03133,.03133,.03133))])
add('same-cell-diagonal',[dict(kind='vertex',p=(-.000049,-.000049,-.000049)),dict(kind='vertex',p=(.000049,.000049,.000049))])
add('place-collision-owner',[dict(kind='vertex',p=(0,0,0)),dict(kind='vertex',p=(1,0,0)),dict(kind='place',vertex=1,p=(.000049,.000049,.000049)),dict(kind='edge',a=(.000049,.000049,.000049),b=(2,0,0))])
if len(sys.argv)>2:cases=[c for c in cases if c['name'] in (( 'exact-half-diagonal',) if sys.argv[2]=='tie' else ('same-cell-diagonal','place-collision-owner'))]
# Only record bulk checkpoints, not quadratically many snapshots of growing meshes.
output=[]
for c in cases:
 if c['name'].startswith(('bulk-','large-')):
  # Input is already captured; avoid rebuilding prefixes in the observer.
  m=Mesh()
  for op in c['operations']:
   f=m.add_face([V(*p) for p in op['outer']],[[V(*p) for p in h]for h in op['holes']]or None);f.attrs.update(op['attrs'])
  states=[fingerprint(m)]
 else:states=run(c)
 output.append(dict(name=c['name'],states=states))
root=Path(sys.argv[1]);root.mkdir(parents=True,exist_ok=True)
(root/'input.json').write_text(json.dumps(cases,separators=(',',':')))
(root/'source.json').write_text(json.dumps(output,separators=(',',':')))
print('captured',len(cases),'cases')
