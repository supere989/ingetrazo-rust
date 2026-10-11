"""Pinned operative console/controller and History ownership, no native simulation."""
import json,sys,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.history import History
from core.ai import run_transactional
from PySide6.QtGui import QVector3D
scene=Scene();scene.mesh.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])
history=History(scene);scope={};viewport=types.SimpleNamespace(scene=scene,history=history,notify_scene_changed=lambda:None)
states=[]
for step in json.loads((Path(__file__).resolve().parents[1]/'docs/audits/ai-console-history/recipes.json').read_text()):
 if step['code']:
  result=run_transactional(viewport,step['code'],scope);assert not result['error'],result
 if step['history'] is not None:assert (history.redo() if step['history'] else history.undo())
 if step['check']:
  result=run_transactional(viewport,step['check'],scope);assert not result['error'],result
 states.append(json.loads(json.dumps(dict(name=step['name'],positions=[[v.position.x(),v.position.y(),v.position.z()]for v in scene.mesh.vertices],attrs=[f.attrs for f in scene.mesh.faces],undo=len(history.undo_stack),redo=len(history.redo_stack),identity_checked=bool(step['check'])))))
print(json.dumps(dict(states=states),indent=2))
