import sys,json,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
rows=[]
for mode in ['false','true','raise','mutating-same','mutating-field','geometry-short-circuit']:
 s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
 def run(code):
  r=run_transactional(vp,code,scope);assert not r.get('error'),r
  return r
 run((Path(__file__).parent/'fixtures/ai-graph-equality-setup.py').read_text())
 run("m.add_face([QVector3D(2,0,0),QVector3D(3,0,0),QVector3D(2,1,0)])");assert h.undo()
 run(f"eq_calls.clear()\nmode={mode!r}")
 try:
  recipe='newp=EqualPayload()\ne.soft=newp'
  if mode=='geometry-short-circuit':recipe+='\nm.vertices[0].position=QVector3D(0,0,2)'
  result={'Ok':run(recipe)['changed']}
 except Exception as exc: result={'Err':f'{type(exc).__name__}: {exc}'}
 e=scope['e'];rows.append(dict(mode=mode,result=result,state=dict(calls=len(scope['eq_calls']),undo=len(h.undo_stack),redo=len(h.redo_stack),payload_is_old=e.soft is scope['oldp'],payload_is_new=e.soft is scope['newp'],hidden=e.hidden)))
print(json.dumps({'cases':rows,'production_enabled':False},indent=2))
