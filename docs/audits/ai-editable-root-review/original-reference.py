import sys,json,types
sys.path.insert(0,'/work/ingetrazo')
from core.ai import run_transactional
from core.scene import Scene
from core.history import History
s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
def run(code,error=False):
 r=run_transactional(vp,code,scope);assert bool(r.get('error'))==error,r
states=[]
def record(name):
 m=scope['m'];ids={id(v):i for i,v in enumerate(m.vertices)};held=scope['held']
 states.append(dict(name=name,mesh=dict(vertices=[[v.position.x(),v.position.y(),v.position.z()]for v in m.vertices],edges=[[ids[id(e.v0)],ids[id(e.v1)]]for e in m.edges],faces=[dict(vertices=[ids[id(v)]for v in f.loop],holes=[[ids[id(v)]for v in ring]for ring in f.hole_loops])for f in m.faces]),persistent_faces=m.faces is scope['faces'],persistent_vertices=m.vertices is scope['vertices'],held_face_live=any(f is held for f in m.faces),face_loop_preserved=held.loop is scope['loop'],attrs_dict_preserved=held.attrs is scope['heldattrs'],vertex_position_preserved=held.loop[0].position is scope['heldposition']))
run("m=mesh\nvertices=m.vertices\nfaces=m.faces\np=QVector3D(0,0,0)\nf=m.add_face([p,QVector3D(1,0,0),QVector3D(0,1,0)])\nheld=f\nloop=f.loop\nheldattrs=f.attrs\nheldposition=f.loop[0].position\nshared=[]\nf.attrs['shared']=shared\nassert f.loop[0].position is not p\nassert f.loop[0].position==p")
record('initial')
run("g=m.add_face([QVector3D(0,0,0),QVector3D(0,1,0),QVector3D(0,0,1)])\nassert g.loop[0] is held.loop[0] and g.loop[1] is held.loop[2]\nassert m.vertices is vertices and m.faces is faces\nassert held.attrs['shared'] is shared")
record('shared')
run("detached=m.add_face([QVector3D(4,0,0),QVector3D(5,0,0),QVector3D(4,1,0)])\nraise ValueError('rollback')",True)
record('abort')
run("assert len(m.faces)==2 and len(m.vertices)==4\nassert detached is not held and detached not in m.faces\nassert detached.loop[0].position==QVector3D(4,0,0)\nassert held.normal()==QVector3D(0,0,1)\nassert held.centroid()==QVector3D(1/3,1/3,0)\nassert held.vertices is not held.vertices\nassert held.vertices[0] is held.loop[0].position")
run("held.attrs[(1,2)]=('kept',shared)\nheld.attrs['vendor']={'opaque':[1,2]}\nbefore_loop=held.loop\nbefore_attrs=held.attrs\nbefore_position=held.loop[0].position")
run("held.attrs[(1,2)]=('wrong',)\nheld.attrs['added']=7\nheld.loop.reverse()\nheld.loop[0].position.setX(12)\nshared.append('nested')\nm.faces.clear()\nm.vertices.clear()\nraise ValueError('field rollback')",True)
run("assert held.attrs[(1,2)][0]=='kept' and held.attrs[(1,2)][1] is shared\nassert held.attrs['vendor']=={'opaque':[1,2]}\nassert 'added' not in held.attrs\nassert shared==['nested']\nassert held.loop is not before_loop and held.attrs is not before_attrs\nassert held.loop[0].position is not before_position\nassert held.normal()==QVector3D(0,0,1)\nassert len(m.faces)==2 and len(m.vertices)==4\nassert m.faces is faces and m.vertices is vertices")
record('field-abort')
run("private=Mesh()\nprivate.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nraise ValueError('private survives')",True)
assert len(scope['private'].faces)==1
print(json.dumps(dict(states=states,face_live_geometry_reads=True,private_scope_edit_survives_error=True,shallow_existing_field_rollback=True),indent=2))
