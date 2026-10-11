import sys,json,types
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
for recipe in json.loads((Path(__file__).parent/'fixtures/ai-graph-callbacks.json').read_text()):
 r=run_transactional(vp,recipe,scope);assert not r.get('error'),r
print('CALLBACK_COUNTS',scope['observed_first'],scope['observed_second'])
