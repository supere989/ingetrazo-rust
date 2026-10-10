import os,sys,json,types
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.history import History
from core.ai import run_transactional
s=Scene();h=History(s);vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None);scope={};states=[]
def record(name):
 vs=list(s.mesh.vertices);ix={id(v):i for i,v in enumerate(vs)}
 states.append(dict(name=name,vertices=[list(v.position.toTuple())for v in vs],edges=[[ix[id(e.v0)],ix[id(e.v1)]]for e in s.mesh.edges],undo=len(h.undo_stack),redo=len(h.redo_stack)))
def run(name,code):
 run_transactional(vp,code,scope);record(name)
run('append','mesh.add_edge(QVector3D(0,0,0), QVector3D(1,0,0))')
h.undo();record('undo')
run('noop-with-redo','sum(range(3))')
run('failure-with-redo','mesh.add_edge(QVector3D(8,0,0), QVector3D(9,0,0))\nraise ValueError("discard")')
h.redo();record('redo')
for i in range(2):
 h.undo();record('repeat-undo-'+str(i));h.redo();record('repeat-redo-'+str(i))
h.undo();record('branch-undo')
run('branch-new','mesh.add_edge(QVector3D(3,0,0), QVector3D(4,0,0))')
h.redo();record('branch-redo-refused')
print(json.dumps(states,indent=2))
