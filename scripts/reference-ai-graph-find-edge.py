"""Operative source incidence-selected lookup, including detached/public-list distinction."""
import json,sys,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
for recipe in json.loads((Path(__file__).parent/'fixtures/ai-graph-find-edge.json').read_text()):
 result=run_transactional(vp,recipe,scope);assert not result.get('error'),result
print(json.dumps({'ordered_callbacks':scope['events'],'undo':len(h.undo_stack),'redo':len(h.redo_stack)}))
