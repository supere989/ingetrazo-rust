import json,sys,types
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
def run(code):
 r=run_transactional(vp,code,scope);assert not r.get('error'),r
run("m=mesh\nf=m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nv=f.loop[0]\nv.edges.clear()")
before=(len(h.undo_stack),len(h.redo_stack));old=scope['v'].edges
run("replacement=set()\nv.edges=replacement")
after=(len(h.undo_stack),len(h.redo_stack))
assert before==after and scope['v'].edges is scope['replacement'] and scope['v'].edges is not old
print(json.dumps({'source_accepts_empty_replacement_without_history':True,'actual_replacement_identity_retained':True,'history_before':before,'history_after':after},indent=2))
