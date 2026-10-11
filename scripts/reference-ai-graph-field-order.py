import sys,json,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
for recipe in json.loads((Path(__file__).parent/'fixtures/ai-graph-field-order.json').read_text()):
 r=run_transactional(vp,recipe,scope);assert not r.get('error'),r
assert len(h.undo_stack)==1 and not h.redo_stack
print(json.dumps({'ordered_callbacks':scope['events'],'undo':1,'redo':0}))
