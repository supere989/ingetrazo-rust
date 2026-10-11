"""Operative pinned source face metadata command history."""
import json,types,sys
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
states=[]
for case in json.loads(Path(__file__).with_name('fixtures').joinpath('ai-face-native-fields.json').read_text()):
 s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
 def run(code):
  result=run_transactional(vp,code,scope);assert not result.get('error'),result
 def observe(phase):
  states.append({'name':case['name']+'/'+phase,'faces':[{'attrs':f.attrs,'layer':f.attrs.get('layer') if isinstance(f.attrs.get('layer'),str) else None}for f in scope['m'].faces],'undo':len(h.undo_stack),'redo':len(h.redo_stack)})
 run('m=mesh\nf=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])');run(case['code']);observe('edited');assert h.undo();observe('undone');assert h.redo();observe('redone')
print(json.dumps({'states':states,'production_enabled':False},indent=2))
