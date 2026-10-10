import sys,json,types
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.history import History
from core.ai import run_transactional
s=Scene();h=History(s);scope={};vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
def run(c):
 r=run_transactional(vp,c,scope)
 assert not r.get('error'),r
run("g=extrude([(0,0),(1,0),(1,1),(0,1)],0,1,name='Authored',color=(0.1,0.2,0.3,0.4))\nheld_mesh=g.mesh\nheld_face=g.mesh.faces[0]\nheld_faces=g.mesh.faces\nassert held_faces is g.mesh.faces")
g=scope['g'];states=[]
def record(name):
 states.append(dict(name=name,groups=len(s.groups),returned_is_placement=any(x is g for x in s.groups),held_mesh_is_group=g.mesh is scope['held_mesh'],held_face_is_mesh_first=g.mesh.faces[0] is scope['held_face'],held_mesh_faces=len(g.mesh.faces),soft_edges=sum(e.soft for e in g.mesh.edges),face_color_types=sorted(set(type(f.attrs['color']).__name__ for f in g.mesh.faces))))
record('finish-group');h.undo();record('finish-undo');scope['held_face'].attrs['held']=['while-undone'];h.redo();record('finish-redo');assert g.mesh.faces[0].attrs['held']==['while-undone']
assert g.mesh.faces is scope['held_faces']
run("color=[1,2,3]\nl=extrude([(0,0),(1,0),(0,1)],0,1,color=color)\nassert l.mesh.faces[0].attrs['color'] is not l.mesh.faces[1].attrs['color']\nt=(1,2,3)\np=extrude([(0,0),(1,0),(0,1)],0,1,color=t)\nassert all(f.attrs['color'] is t for f in p.mesh.faces)\ni=iter([1,2,3])\nq=extrude([(0,0),(1,0),(0,1)],0,1,color=i)\nassert q.mesh.faces[0].attrs['color']==(1,2,3)\nassert all(f.attrs['color']==() for f in q.mesh.faces[1:])\nname=['arbitrary-name']\nr=extrude([(0,0),(1,0),(0,1)],0,1,name=name)\nassert r.name is name")
geometry=[]
for name,recipe in [('ccw',"extrude([(0,0),(1,0),(1,1),(0,1)],0,1,name='explicit')"),('cw',"extrude([(0,1),(1,1),(1,0),(0,0)],0,1,name='explicit')"),('negative-height',"extrude([(0,0),(1,0),(1,1),(0,1)],2,-1,name='explicit')"),('flat',"extrude([(0,0),(1,0),(1,1),(0,1)],0,0,name='explicit')"),('empty',"extrude([],0,1,name='explicit')")]:
 run('g='+recipe)
 m=scope['g'].mesh;ids={id(v):i for i,v in enumerate(m.vertices)}
 geometry.append(dict(name=name,mesh=dict(vertices=[[v.position.x(),v.position.y(),v.position.z()]for v in m.vertices],edges=[dict(vertices=[ids[id(e.v0)],ids[id(e.v1)]],soft=e.soft)for e in m.edges],faces=[dict(vertices=[ids[id(v)]for v in f.loop],interior=f.interior)for f in m.faces])))
r=run_transactional(vp,"extrude(iter([(0,0),(1,0),(0,1)]),0,1)",scope)
assert r.get('error')=='SnapshotImport: list index out of range',r
run('g=extrude([],0,1)');n=int(scope['g'].name[6:])
s2=Scene();h2=History(s2);scope2={};vp2=types.SimpleNamespace(scene=s2,history=h2,notify_scene_changed=lambda:None)
r=run_transactional(vp2,'g=extrude([],0,1)',scope2);assert not r.get('error'),r
assert int(scope2['g'].name[6:])==n+1
s3=Scene();h3=History(s3);scope3={};vp3=types.SimpleNamespace(scene=s3,history=h3,notify_scene_changed=lambda:None)
r=run_transactional(vp3,"earlier=extrude([(0,0),(1,0),(0,1)],0,1,name='earlier')\ntry:\n    extrude(iter([(0,0),(1,0),(0,1)]),0,1)\nexcept IndexError:\n    pass",scope3)
assert not r.get('error') and r['changed'] and len(s3.groups)==1,r
s4=Scene();h4=History(s4);scope4={};vp4=types.SimpleNamespace(scene=s4,history=h4,notify_scene_changed=lambda:None)
r=run_transactional(vp4,"tuple=lambda x:'wrong'\nlist=lambda x:'wrong'\nlen=lambda x:0\nrange=lambda n:[]\ng=extrude([(0,0),(1,0),(0,1)],0,1,name='shadow',color=[1,2,3])\nassert g.mesh.faces[0].attrs['color']==(1,2,3)",scope4)
assert not r.get('error') and len(scope4['g'].mesh.faces)==5,r
print(json.dumps(dict(group_states=states,geometry=geometry,held_mutation_visible_after_redo=True,persistent_mesh_faces_list_identity=True,per_face_color_conversion=True,helper_globals_not_user_shadowed=True,arbitrary_truthy_name_identity=True,outline_double_iteration=True,cross_session_name_counter=True,caught_python_helper_error_preserves_prior_group=True),indent=2))
