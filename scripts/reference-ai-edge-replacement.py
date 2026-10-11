"""Operative frozen run_transactional public Edge replacement/history oracle."""
import json,sys,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
cases=json.loads((Path(__file__).parent/'fixtures/ai-edge-replacement.json').read_text())
states=[]
for case in cases:
 s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
 def run(code):
  result=run_transactional(vp,code,scope);assert not result.get('error'),result
 run('m=mesh\nf=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nv0=m.vertices[0]\nv1=m.vertices[1]\nv2=m.vertices[2]\ne=m.edges[0]\noldedges=list(m.edges)')
 m=scope['m']
 def observe(phase):
  def ident(v,items):return next((i for i,x in enumerate(items)if x is v),None)
  vertices=m.vertices;edges=m.edges;faces=m.faces
  positions=[[v.position.x(),v.position.y(),v.position.z()]for v in vertices]
  endpoints=[[ident(e.v0,vertices),ident(e.v1,vertices)]for e in edges]
  incidence=[sorted([ident(e,edges)for e in v.edges],key=lambda x:-1 if x is None else x)for v in vertices]
  radial=[[ident(f,faces)for f in e.faces]for e in edges]
  rings=[{'loop':[ident(v,vertices)for v in f.loop],'holes':[[ident(v,vertices)for v in ring]for ring in f.hole_loops]}for f in faces]
  packed_incidence=[[]for v in vertices]
  for i,(a,b)in enumerate(endpoints):
   packed_incidence[a].append(i)
   if b!=a:packed_incidence[b].append(i)
  states.append({'name':case['name']+'/'+phase,'positions':positions,'endpoints':endpoints,'incidence':incidence,'radial':radial,'faces':rings,'native_radial':radial,'undo':len(h.undo_stack),'redo':len(h.redo_stack),'native_endpoints':endpoints,'native_incidence':packed_incidence,'native_positions':positions,'native_faces':rings,'held_edge_live':any(x is scope['e'] for x in edges),'held_endpoint_ids':[ident(scope['e'].v0,vertices),ident(scope['e'].v1,vertices)],'held_edge_incidence':[scope['e'] in v.edges for v in vertices],'native_metadata':[{'soft':e.soft,'hidden':e.hidden,'layer':e.layer,'curve':e.curve}for e in edges]})
 run(case['recipe']);observe('edited');assert h.undo();assert m.edges[0]is scope['e'];observe('undone');assert h.redo();observe('redone')
print(json.dumps({'states':states,'production_enabled':False},indent=2))
