"""Actual pinned Scene/SnapshotImport group membership law."""
import json,sys,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.history import History
from core.mesh import Mesh
from core.group import Group
from PySide6.QtGui import QVector3D
from core.ai import run_transactional
scene=Scene(); em=Mesh(); em.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)]); scene.groups.append(Group(em,name='Existing'));history=History(scene);scope={}
viewport=types.SimpleNamespace(scene=scene,history=history,notify_scene_changed=lambda:None)
states=[]
for s in json.loads((Path(__file__).resolve().parents[1]/'docs/audits/ai-console-groups/recipes.json').read_text()):
 if s['code']:
  result=run_transactional(viewport,s['code'],scope);assert not result['error'],result
 if s['history'] is not None:assert (history.redo() if s['history'] else history.undo())
 # Observe before inspecting aliases; the inspection must never repair publication.
 state=dict(name=s['name'],groups=[dict(name=g.name,layer=g.layer,hidden=g.hidden,positions=[[v.position.x(),v.position.y(),v.position.z()] for v in g.mesh.vertices],attrs=[f.attrs for f in g.mesh.faces]) for g in scene.groups],undo=len(history.undo_stack),redo=len(history.redo_stack))
 if s['check']:
  result=run_transactional(viewport,s['check'],scope);assert not result['error'],result
 states.append(json.loads(json.dumps(state)))
print(json.dumps(dict(states=states),indent=2))
