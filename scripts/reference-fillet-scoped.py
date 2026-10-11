#!/usr/bin/env python3
"""Frozen live-object source Fillet ownership corpus; no serialized oracle."""
import json,sys,hashlib
from pathlib import Path
from PySide6.QtGui import QVector3D as V
from core.mesh import Mesh
from core.fillet import plan_fillet,apply_fillet
from tests.test_fillet import box,edge_at
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
def snap(m,before=None):
 v={id(v):i for i,v in enumerate(m.vertices)}
 old=before or ([],[],[])
 return {'vertices':[[v.position.x(),v.position.y(),v.position.z()]for v in m.vertices],
 'vertex_origins':[next((i for i,x in enumerate(old[0])if x is v),None)for v in m.vertices],
 'edges':[{'ends':[v[id(e.v0)],v[id(e.v1)]],'soft':e.soft,'curve':e.curve,'layer':e.layer,'hidden':e.hidden,'attrs':getattr(e,'attrs',{}),'origin':next((i for i,x in enumerate(old[1])if x is e),None)}for e in m.edges],
 'faces':[{'loop':[v[id(x)]for x in f.loop],'holes':[[v[id(x)]for x in h]for h in f.hole_loops],'attrs':f.attrs,'interior':f.interior,'origin':next((i for i,x in enumerate(old[2])if x is f),None)}for f in m.faces]}
rows=[]
for name in ['endpoint-free-wire','open-neighbor','paint-first-rim','oblique-cap','concave','turned-squashed']:
 m=box();radius=.2;segments=8
 if name=='oblique-cap':
  for v in m.vertices:
   if v.position.x()==2:m.place_vertex(v,V(2.5 if v.position.y()==0 else 2,v.position.y(),v.position.z()))
 if name=='concave':
  m=Mesh();foot=[V(0,0,0),V(2,0,0),V(2,1,0),V(1,1,0),V(1,2,0),V(0,2,0)];m.add_face(list(reversed(foot)));m.add_face([V(q.x(),q.y(),1)for q in foot]);
  for a,b in zip(foot,foot[1:]+foot[:1]):m.add_face([a,b,V(b.x(),b.y(),1),V(a.x(),a.y(),1)])
 if name=='turned-squashed':
  from PySide6.QtGui import QMatrix4x4
  matrix=QMatrix4x4();matrix.translate(3,-5,7);matrix.rotate(90,0,1,0);matrix.scale(2,3,.5)
  for v in list(m.vertices):m.place_vertex(v,matrix.map(v.position))
 # Capture explicit selected edge order, independent of source selection sets.
 if name=='concave':edges=[edge_at(m,(1,1,0),(1,1,1))]
 elif name=='turned-squashed':edges=list(m.edges);radius=.1;segments=4
 else:
  e=next(e for e in m.edges if e.a.y()==e.b.y()==0 and e.a.z()==e.b.z()==1)
  edges=[e]
 if name=='paint-first-rim':
  for i,f in enumerate(m.faces):f.attrs={'mat':'stone' if i in (1,2)else 'wood','vendor':{'face':i},'layer':'tag'}
  edges=[e]+[x for x in m.edges if x is not e and x.a.z()==x.b.z()==1]
 if name in ('endpoint-free-wire','open-neighbor'):
  a=edges[0].a;m.add_edge(a,V(a.x(),a.y()-2,a.z()+2));m.add_edge(V(10,0,0),V(11,0,0));m.vertex(V(17,3,2))
  if name=='open-neighbor':
   f=m.add_face([V(8,0,0),V(9,0,0),V(9,1,0)]);f.attrs={'vendor':'parked','color':[.2,.3,.4]};f.interior=True
  for i,e in enumerate(m.edges):e.hidden=i%2==0;e.layer='old';e.curve=41 if not e.faces else None
  m.faces[1].attrs={'mat':'stone','vendor':'cut','layer':'tag'};m.faces[1].interior=True
 before=(list(m.vertices),list(m.edges),list(m.faces));source=snap(m,before);selected=[before[1].index(e)for e in edges]
 plan=plan_fillet(m,edges,radius,segments);assert not isinstance(plan,str),(name,plan)
 apply_fillet(m,plan);rows.append({'name':name,'radius':radius,'segments':segments,'selected':selected,'before':source,'after':snap(m,before)})
Path(sys.argv[1]).write_text(json.dumps({'source_pin':PIN,'cases':rows,'source_files':{p:hashlib.sha256(Path(p).read_bytes()).hexdigest()for p in ['core/fillet.py','core/mesh.py','tools/fillet.py']}},indent=2)+'\n')
