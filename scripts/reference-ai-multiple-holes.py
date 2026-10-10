import os,sys,json,types
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtGui import QVector3D
from core.scene import Scene
from core.history import History
from core.ai import run_transactional
cases=json.loads((Path(__file__).resolve().parents[1]/'tools/ai-interpreter-proof/face-multiple-hole-cases.json').read_text());states=[]
def record(name):
 m=s.mesh;vs=list(m.vertices);ix={id(v):i for i,v in enumerate(vs)};ei={id(e):i for i,e in enumerate(m.edges)};fi={id(f):i for i,f in enumerate(m.faces)}
 states.append(dict(name=name,undo=len(h.undo_stack),redo=len(h.redo_stack),adjacency=[sorted(ei[id(e)]for e in v.edges)for v in vs],vertices=[list(v.position.toTuple())for v in vs],edges=[dict(vertices=[ix[id(e.v0)],ix[id(e.v1)]],faces=[fi[id(f)]for f in e.faces])for e in m.edges],faces=[dict(outer=[ix[id(v)]for v in f.loop],holes=[[ix[id(v)]for v in ring]for ring in f.hole_loops])for f in m.faces]))
for c in cases:
 s=Scene();h=History(s);vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None);scope={'outer':c['outer'],'holes':c['holes']}
 for p in c.get('seed_vertices',[]):s.mesh.vertex(QVector3D(*p))
 result=run_transactional(vp,'mesh.add_face([QVector3D(*p)for p in outer],[[QVector3D(*p)for p in ring]for ring in holes])',scope)
 assert not result.get('error'),result
 record(c['name']);h.undo();record(c['name']+'-undo');h.redo();record(c['name']+'-redo')
print(json.dumps(states,indent=2))
