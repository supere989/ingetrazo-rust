import os,sys,json,types
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.mesh import Mesh
from core.history import History
from core.ai import run_transactional
s=Scene();h=History(s);vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None);scope={};states=[]
def record(name,group=False):
 m=(s.groups[-1].mesh if s.groups else Mesh())if group else s.mesh
 vs=list(m.vertices);ix={id(v):i for i,v in enumerate(vs)};fi={id(f):i for i,f in enumerate(m.faces)}
 states.append(dict(name=name,vertices=[list(v.position.toTuple())for v in vs],edges=[dict(vertices=[ix[id(e.v0)],ix[id(e.v1)]],faces=[fi[id(f)]for f in e.faces])for e in m.edges],faces=[dict(outer=[ix[id(v)]for v in f.loop],holes=[[ix[id(v)]for v in ring]for ring in f.hole_loops])for f in m.faces]))
def run(code):
 result=run_transactional(vp,code,scope)
 if result.get('error'):raise RuntimeError(result)
record('empty')
run('mesh.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])');record('triangle')
run('mesh.add_face([QVector3D(1,0,0),QVector3D(0,0,0),QVector3D(0,-1,0)])');record('shared');h.undo();record('shared-undo');h.redo();record('shared-redo')
run('mesh.add_face([QVector3D(0,0,1),QVector3D(4,0,1),QVector3D(4,4,1),QVector3D(0,4,1)],[[QVector3D(1,1,1),QVector3D(1,2,1),QVector3D(2,2,1),QVector3D(2,1,1)]])');record('hole');h.undo();record('hole-undo');h.redo();record('hole-redo')
failure=run_transactional(vp,'mesh.add_face([QVector3D(30,0,0),QVector3D(31,0,0),QVector3D(30,1,0)])\nraise ValueError(\"discard\")',scope)
assert failure.get('error');record('explicit-error-rollback')
for name,sign in [('slanted',1),('reflected',-1)]:
 run(f'mesh.add_face([QVector3D({sign*20.125},0.2,2.3),QVector3D({sign*21.125},0.2,2.8),QVector3D({sign*21.125},1.2,3.3),QVector3D({sign*20.125},1.2,2.8)])');record(name);h.undo();record(name+'-undo');h.redo();record(name+'-redo')
s=Scene();h=History(s);vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None);scope={}
run('extrude([(10,0),(12,0),(12,1),(10,1)],0.25,1.75)');record('extrude-geometry',True);h.undo();record('extrude-undo',True);h.redo();record('extrude-redo',True)
print(json.dumps(states,indent=2))
