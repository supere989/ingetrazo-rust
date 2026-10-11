"""Operative pinned recipe helpers and SnapshotImport membership."""
import json,sys,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.history import History
from core.ai import run_transactional
scene=Scene();history=History(scene);scope={};v=types.SimpleNamespace(scene=scene,history=history,notify_scene_changed=lambda:None)
states=[]
for s in json.loads((Path(__file__).resolve().parents[1]/'docs/audits/ai-console-house/recipes.json').read_text()):
 if s['code']:
  result=run_transactional(v,s['code'],scope);assert bool(result['error'])==s.get('expect_error',False),result
 if s['history'] is not None:assert (history.redo() if s['history'] else history.undo())
 state=dict(name=s['name'],groups=[dict(name=g.name,positions=[[p.position.x(),p.position.y(),p.position.z()] for p in g.mesh.vertices],faces=[dict(vertices=[g.mesh.vertices.index(x) for x in f.loop],holes=[[g.mesh.vertices.index(x) for x in h] for h in f.hole_loops],attrs=f.attrs) for f in g.mesh.faces],edges=[dict(vertices=[g.mesh.vertices.index(e.v0),g.mesh.vertices.index(e.v1)],soft=e.soft) for e in g.mesh.edges]) for g in scene.groups],undo=len(history.undo_stack),redo=len(history.redo_stack))
 if s['check']:
  result=run_transactional(v,s['check'],scope);assert not result['error'],result
 states.append(json.loads(json.dumps(state)))
print(json.dumps(dict(states=states),indent=2))
