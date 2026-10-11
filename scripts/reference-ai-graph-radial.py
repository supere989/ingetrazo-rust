"""Actual source per-boundary radial multiplicity, including legal degenerate faces."""
import json,sys,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
states=[]
for case in json.loads((Path(__file__).parent/'fixtures/ai-graph-radial.json').read_text()):
 s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
 result=run_transactional(vp,case['recipe'],scope);assert not result.get('error'),result
 m=scope['m']
 def ident(value,items):return next((i for i,x in enumerate(items)if value is x),None)
 def record(phase):
  radial=[[ident(f,m.faces)for f in e.faces]for e in m.edges]
  states.append({'name':case['name']+'/'+phase,'positions':[[v.position.x(),v.position.y(),v.position.z()]for v in m.vertices],'endpoints':[[ident(e.v0,m.vertices),ident(e.v1,m.vertices)]for e in m.edges],'incidence':[sorted(ident(e,m.edges)for e in v.edges)for v in m.vertices],'radial':radial,'faces':[{'loop':[ident(v,m.vertices)for v in f.loop],'holes':[[ident(v,m.vertices)for v in ring]for ring in f.hole_loops]}for f in m.faces],'native_radial':radial,'undo':len(h.undo_stack),'redo':len(h.redo_stack)})
 record('edited');assert h.undo();record('undone');assert h.redo();record('redone')
print(json.dumps({'states':states,'production_enabled':False},indent=2))
