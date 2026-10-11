"""Operative pinned recipe helpers and SnapshotImport membership."""
import json,sys,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.history import History
from core.ai import run_transactional
from core.mesh import Mesh
from core.group import Group
from PySide6.QtGui import QMatrix4x4,QVector3D
from formats.igz import save_scene,load_into
seed=Scene();m=Mesh();m.add_face([QVector3D(*p) for p in [(0,0,0),(1,0,0),(1,1,0),(0,1,0)]])
for name in ['A','B']:
 g=Group(m,name=name);g.xform=QMatrix4x4();g.component=True;seed.groups.append(g)
save_scene(seed,Path(sys.argv[1]));scene=Scene();load_into(scene,Path(sys.argv[1]));history=History(scene);scope={};v=types.SimpleNamespace(scene=scene,history=history,notify_scene_changed=lambda:None)
states=[]
for s in json.loads((Path(__file__).resolve().parents[1]/'docs/audits/ai-console-imported-mesh/recipes.json').read_text()):
 if s['code']:
  result=run_transactional(v,s['code'],scope);assert bool(result['error'])==s.get('expect_error',False),result
 if s['history'] is not None:assert (history.redo() if s['history'] else history.undo())
 owners=[]
 for g in scene.groups:
  if not any(g.mesh is x for x in owners):owners.append(g.mesh)
 state=dict(packed_meshes=len(owners),aliases=[[g.mesh is h.mesh for h in scene.groups] for g in scene.groups],name=s['name'],groups=[dict(name=g.name,component=g.is_component(),placed=g.xform is not None,positions=[[p.position.x(),p.position.y(),p.position.z()] for p in g.mesh.vertices],faces=[dict(vertices=[g.mesh.vertices.index(x) for x in f.loop],holes=[[g.mesh.vertices.index(x) for x in h] for h in f.hole_loops],attrs=f.attrs) for f in g.mesh.faces],edges=[dict(vertices=[g.mesh.vertices.index(e.v0),g.mesh.vertices.index(e.v1)],soft=e.soft) for e in g.mesh.edges]) for g in scene.groups],undo=len(history.undo_stack),redo=len(history.redo_stack))
 if s['name']=='reassignment':save_scene(scene,Path(sys.argv[1]+'.source-replacement'))
 if s['check']:
  result=run_transactional(v,s['check'],scope);assert not result['error'],result
 states.append(json.loads(json.dumps(state)))
save_scene(scene,Path(sys.argv[1]+'.source-final'))
print(json.dumps(dict(states=states),indent=2))
