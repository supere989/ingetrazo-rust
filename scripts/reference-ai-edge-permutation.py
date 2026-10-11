"""Actual source public ordering and current live identity geometry."""
import json,sys,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
states=[]
for case in json.loads((Path(__file__).parent/'fixtures/ai-edge-permutation.json').read_text()):
 s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
 def run(recipe):
  result=run_transactional(vp,recipe,scope);assert not result.get('error'),result
 run('m=mesh\nf0=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nf1=m.add_face([QVector3D(1,0,0),QVector3D(),QVector3D(0,0,1)])')
 m=scope['m']
 def ident(value,items):return next((i for i,x in enumerate(items)if x is value),None)
 def record(phase):
  positions=[[v.position.x(),v.position.y(),v.position.z()]for v in m.vertices]
  endpoints=[[ident(e.v0,m.vertices),ident(e.v1,m.vertices)]for e in m.edges]
  incidence=[sorted(ident(e,m.edges)for e in v.edges)for v in m.vertices]
  radial=[[ident(f,m.faces)for f in e.faces]for e in m.edges]
  faces=[{'loop':[ident(v,m.vertices)for v in f.loop],'holes':[[ident(v,m.vertices)for v in ring]for ring in f.hole_loops]}for f in m.faces]
  states.append({'name':case['name']+'/'+phase,'positions':positions,'endpoints':endpoints,'incidence':incidence,'radial':radial,'faces':faces,'native_radial':radial,'native_endpoints':endpoints,'native_incidence':incidence,'native_positions':positions,'native_faces':faces,'undo':len(h.undo_stack),'redo':len(h.redo_stack)})
 run(case['recipe']);record('edited');assert h.undo();record('undone');assert h.redo();record('redone');assert h.undo();record('repeat-undone');assert h.redo();record('repeat-redone')
 run('f2=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(2,2,2)])');record('append-shared');assert h.undo();record('append-undone');assert h.undo();record('chronological-order-undone');assert h.redo();record('chronological-order-redone');assert h.redo();record('append-redone')
print(json.dumps({'states':states,'production_enabled':False},indent=2))
