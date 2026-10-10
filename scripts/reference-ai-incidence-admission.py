"""Original accepts root incidence edits; native temporary refusal is not parity."""
import json,sys,types
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
def run(code):
 result=run_transactional(vp,code,scope);assert not result.get('error'),result
run("m=mesh\nheld=m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nv=held.loop[0]\noldedges=v.edges")
states=[]
def record(name):
 v=scope['v']; e=scope.get('e');states.append(dict(name=name,position=list(v.position.toTuple()),incidence_count=len(v.edges),new_edge_in_incidence=e in v.edges if e is not None else False,old_set_same=v.edges is scope['oldedges'],old_set_has_new=e in scope['oldedges'] if e is not None else False,undo=len(h.undo_stack),redo=len(h.redo_stack)))
record('initial')
run("e=Edge(v,held.loop[1])\nv.edges.add(e)\nv.position.setX(9)");record('accepted-incidence')
assert h.undo();record('undone')
assert h.redo();record('redone')
print(json.dumps({'states':states,'source_accepts_incidence':True,'native_temporary_refusal_is_parity':False},indent=2))
