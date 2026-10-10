import os,sys,json,types
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.history import History
from core.ai import run_transactional
s=Scene();h=History(s);vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None);scope={};states=[]
code="""shared=[]
a=mesh.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])
b=mesh.add_face([QVector3D(0,0,1),QVector3D(1,0,1),QVector3D(0,1,1)])
a.attrs['shared']=shared
b.attrs['shared']=shared
a.attrs['non_string_keys']={17:'integer',(1,2):'tuple'}
a.attrs['tuple']=(1,2,3)
"""
r=run_transactional(vp,code,scope);assert not r.get('error'),r
held=scope['a'];shared=scope['shared']
def record(name):
 fs=list(s.mesh.faces);states.append(dict(name=name,faces=len(fs),held_is_live=any(f is held for f in fs),shared_alias=(len(fs)==2 and fs[0].attrs['shared'] is fs[1].attrs['shared']),shared_is_scope=(bool(fs) and fs[0].attrs['shared'] is shared),tuple_type=(type(fs[0].attrs['tuple']).__name__ if fs else None),held_shared_value=list(held.attrs['shared']),scope_shared_value=list(shared),live_shared_values=[list(f.attrs['shared'])for f in fs],nonstring_keys=(sorted(type(k).__name__ for k in fs[0].attrs['non_string_keys']) if fs else [])))
record('append');h.undo();record('undo');shared.append('while-undone');record('scope-mutated-while-undone');h.redo();record('redo');shared.append('later');record('scope-alias-mutated')
s=Scene();h=History(s);vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None);scope={}
r=run_transactional(vp,"g=extrude([(0,0),(1,0),(1,1),(0,1)],0,1,name='Authored',color=(0.1,0.2,0.3,0.4))",scope);assert not r.get('error'),r
g=scope['g'];states.append(dict(name='finish-group',returned_is_placement=s.groups[-1] is g,face_color_types=sorted(set(type(f.attrs['color']).__name__ for f in g.mesh.faces)),soft_edges=sum(e.soft for e in g.mesh.edges)))
h.undo();states.append(dict(name='finish-undo',groups=len(s.groups),held_mesh_faces=len(g.mesh.faces)));h.redo();states.append(dict(name='finish-redo',returned_is_placement=s.groups[-1] is g,groups=len(s.groups)))
print(json.dumps(states,indent=2))
