import os,sys,json,types
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.history import History
from core.ai import run_transactional
s=Scene();h=History(s);vp=types.SimpleNamespace(scene=s,history=h,notify_scene_changed=lambda:None)
code='mesh.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)])\nmesh.add_face([QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)],[[],[QVector3D(0,0,0),QVector3D(1,0,0),QVector3D(0,1,0)]])'
result=run_transactional(vp,code,{})
assert result['error']=='SnapshotImport: list index out of range' and 'IndexError' in result['stderr'],result
print(json.dumps({'document':{'vertices':len(s.mesh.vertices),'edges':len(s.mesh.edges),'faces':len(s.mesh.faces),'undo':len(h.undo_stack),'redo':len(h.redo_stack)},'raw_result':result},indent=2))
