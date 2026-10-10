import json,sys,types
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
from PySide6.QtGui import QVector3D
s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
def run(code,error=False):
 r=run_transactional(vp,code,scope);assert bool(r.get('error')) is error,r
 return r
states=[]
def record(name):
 m=scope['m'];f=scope['held'];v=scope['v'];ids={id(x):i for i,x in enumerate(m.vertices)}
 states.append(dict(name=name,positions=[[x.position.x(),x.position.y(),x.position.z()] for x in m.vertices],faces=[[ids[id(x)]for x in y.loop]for y in m.faces],face_live=any(x is f for x in m.faces),vertex_live=any(x is v for x in m.vertices),loop_original=f.loop is scope['oldloop'],position_original=v.position is scope['oldposition'],attrs_original=f.attrs is scope['oldattrs'],shared_value=list(f.attrs['shared']),shared_identity=f.attrs['shared'] is scope['shared'],held_position=[scope['oldposition'].x(),scope['oldposition'].y(),scope['oldposition'].z()],undo=len(h.undo_stack),redo=len(h.redo_stack)))
run("m=mesh\nheld=m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nv=held.loop[0]\noldloop=held.loop\noldposition=v.position\noldattrs=held.attrs\nshared=[]\nheld.attrs['shared']=shared")
record('initial')
run("v.position.setX(2)\nshared.append('live')\nadded=m.add_face([QVector3D(0,0,0),QVector3D(0,0,1),QVector3D(0,1,0)])\nheld.loop[1]=added.loop[1]")
run("held_added_vertex=added.loop[1]\nassert held.loop[1] is held_added_vertex");record('mixed')
assert h.undo();run("assert all(f is not added for f in m.faces)\nassert held.loop[1] is not held_added_vertex\nassert held_added_vertex.position.z()==1");record('undone')
assert h.redo();run("assert added is m.faces[-1]\nassert held.loop[1] is held_added_vertex\nassert added.loop[1] is held_added_vertex");record('redone')
assert h.undo();record('undone-again')
assert h.redo();record('redone-again')
assert h.undo();counts=(len(h.undo_stack),len(h.redo_stack));run("v.position.setZ(7)\nfailed=m.add_face([QVector3D(3,0,0),QVector3D(4,0,0),QVector3D(3,1,0)])\nraise ValueError('mixed rollback')",True);assert (len(h.undo_stack),len(h.redo_stack))==counts;assert h.redo()
assert h.undo();assert h.undo();assert len(s.mesh.faces)==0;assert h.redo();assert h.redo()
print(json.dumps({'states':states,'appended_reference_identity':True,'chronological_undo_redo':True,'execution_refusal_preserves_redo':True},indent=2))
