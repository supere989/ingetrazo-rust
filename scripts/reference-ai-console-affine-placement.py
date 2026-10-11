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
 g=Group(m,name=name);g.xform=QMatrix4x4();g.xform.translate(.5,-.75,1) if name=='A' else g.xform.scale(-1,2,.5);g.component=True;seed.groups.append(g)
save_scene(seed,Path(sys.argv[1]));scene=Scene();load_into(scene,Path(sys.argv[1]));history=History(scene);scope={};v=types.SimpleNamespace(scene=scene,history=history,notify_scene_changed=lambda:None)
states=[]
for s in json.loads((Path(__file__).resolve().parents[1]/'docs/audits/ai-console-affine-placement/recipes.json').read_text()):
 if s['code']:
  result=run_transactional(v,s['code'],scope);assert bool(result['error'])==s.get('expect_error',False),result
 if s['history'] is not None:assert (history.redo() if s['history'] else history.undo())
 state=dict(name=s['name'],aliases=[[g.mesh is h.mesh for h in scene.groups] for g in scene.groups],transforms=[list(g.xform.data()) if g.xform is not None else None for g in scene.groups],names=[g.name for g in scene.groups],components=[g.is_component() for g in scene.groups],undo=len(history.undo_stack),redo=len(history.redo_stack))
 if s['name']=='reassignment':save_scene(scene,Path(sys.argv[1]+'.source-replacement'))
 if s.get('check'):
  result=run_transactional(v,s['check'],scope);assert not result['error'],result
 states.append(json.loads(json.dumps(state)))
save_scene(scene,Path(sys.argv[1]+'.source-final'))
print(json.dumps(dict(states=states),indent=2))
