"""Actual pinned source forward mesh command and history observations."""
import json,sys,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.history import History
from core.ai import run_transactional
from PySide6.QtGui import QVector3D
scene=Scene();face=scene.mesh.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])
face.attrs.update({'color':[.2,.4,.8],'layer':'Imported','vendor':{'unit':[1,2],'name':'kept'}})
history=History(scene);scope={};viewport=types.SimpleNamespace(scene=scene,history=history,notify_scene_changed=lambda:None)
states=[]
for step in json.loads((Path(__file__).resolve().parents[1]/'docs/audits/ai-console-rows/recipes.json').read_text()):
 if step['code']:
  result=run_transactional(viewport,step['code'],scope);assert not result['error'],result
 if step['history'] is not None:assert (history.redo() if step['history'] else history.undo())
 vi={v:i for i,v in enumerate(scene.mesh.vertices)};fi={f:i for i,f in enumerate(scene.mesh.faces)}
 states.append(json.loads(json.dumps(dict(name=step['name'],positions=[[v.position.x(),v.position.y(),v.position.z()]for v in scene.mesh.vertices],edges=[dict(v0=vi[e.v0],v1=vi[e.v1],faces=[fi[f]for f in e.faces])for e in scene.mesh.edges],faces=[dict(vertices=[vi[v]for v in f.loop],holes=[[vi[v]for v in h]for h in f.holes],attrs=f.attrs,layer=f.attrs.get('layer'))for f in scene.mesh.faces],layers=[l.name for l in scene.layers],undo=len(history.undo_stack),redo=len(history.redo_stack),identity_checked=bool(step['check'])))))
 if step['check']:
  result=run_transactional(viewport,step['check'],scope);assert not result['error'],result
print(json.dumps(dict(states=states),indent=2))
