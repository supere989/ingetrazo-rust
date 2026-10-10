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
run("m=mesh\nheld=m.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nv=held.loop[0]\nvertices_list=m.vertices\nedges_list=m.edges\nfaces_list=m.faces\noldposition=v.position\noldvedges=v.edges\nheldedge=m.edges[0]\noldefaces=heldedge.faces")
m=scope['m'];labels={id(v):i for i,v in enumerate(m.vertices)}
def record(name):
 ids=lambda xs:[labels[id(v)]for v in xs]
 v=scope['v'];e=scope['heldedge']
 states.append(dict(name=name,vertices=ids(m.vertices),positions=[[v.position.x(),v.position.y(),v.position.z()]for v in m.vertices],faces=[ids(f.loop)for f in m.faces],edge_endpoints=[[labels[id(e.v0)],labels[id(e.v1)]]for e in m.edges],registry_zero_is_held=m.vertex_at(QVector3D(0,0,0)) is v,held_in_membership=any(x is v for x in m.vertices),held_in_loop=any(x is v for x in scope['held'].loop),persistent_lists=[m.vertices is scope['vertices_list'],m.edges is scope['edges_list'],m.faces is scope['faces_list']],held_position_original=v.position is scope['oldposition'],held_incidence_original=v.edges is scope['oldvedges'],edge_radial_original=e.faces is scope['oldefaces'],undo=len(h.undo_stack),redo=len(h.redo_stack)))
record('initial')
run("m.vertices.reverse()");record('reordered')
assert h.undo();record('reorder-undone')
assert h.redo();record('reorder-redone')
run("m.vertices.remove(v)");record('removed')
assert h.undo();record('remove-undone')
assert h.redo();record('remove-redone')
run("m.vertices.append(v)");record('reinserted')
assert h.undo();record('reinsert-undone')
assert h.redo();record('reinsert-redone')
print(json.dumps({'states':states,'native_acceptance':False},indent=2))
