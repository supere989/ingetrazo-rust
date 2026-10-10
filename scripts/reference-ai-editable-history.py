import json,sys,types
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
from PySide6.QtGui import QVector3D
s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
def run(code):
 r=run_transactional(vp,code,scope);assert not r.get('error'),r
states=[]
def record(name):
 m=scope['m'];f=scope['held'];v=scope['v'];ids={id(x):i for i,x in enumerate(m.vertices)}
 states.append(dict(name=name,positions=[[x.position.x(),x.position.y(),x.position.z()] for x in m.vertices],faces=[[ids[id(x)]for x in y.loop]for y in m.faces],face_live=any(x is f for x in m.faces),vertex_live=any(x is v for x in m.vertices),loop_original=f.loop is scope['oldloop'],position_original=v.position is scope['oldposition'],attrs_original=f.attrs is scope['oldattrs'],shared_value=list(f.attrs['shared']),shared_identity=f.attrs['shared'] is scope['shared'],held_position=[scope['oldposition'].x(),scope['oldposition'].y(),scope['oldposition'].z()],undo=len(h.undo_stack),redo=len(h.redo_stack)))
run("m=mesh\nheld=m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nv=held.loop[0]\noldloop=held.loop\noldposition=v.position\noldattrs=held.attrs\nshared=[]\nheld.attrs['shared']=shared")
record('initial')
run("v.position.setX(2)\nheld.loop.reverse()\nshared.append('live')")
record('edited')
assert h.undo();record('undone')
assert h.redo();record('redone')
assert h.undo();record('undone-again')
assert h.redo();record('redone-again')
old=s.mesh.vertex(QVector3D(0,0,0));new=s.mesh.vertex(QVector3D(2,0,0))
print(json.dumps({'states':states,'old_key_returns_moved_identity':old is scope['v'],'new_key_creates_distinct_coincident_vertex':new is not scope['v'],'source_registry_rebuilt_on_vector_mutation':False},indent=2))
