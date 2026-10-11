import os,sys,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D
def V(x=0,y=0,z=0):return QVector3D(x,y,z)
from core.scene import Scene
from core.history import History,run_stitch
from core.mesh import _key
from tests.test_pushpull_ux import _cube,_top,_locked_tool,_StubViewport,_push
app=QApplication.instance()or QApplication([])
def point(p):return [p.x(),p.y(),p.z()]
def snapshot(m,origins=None):return {"faces":[{"vertices":[point(p)for p in f.vertices],"holes":[[point(p)for p in h]for h in f.holes],"attrs":dict(f.attrs),"interior":bool(f.interior)}for f in m.faces],"vertices":[point(v.position)for v in m.vertices],"vertex_origins":[None if origins is None else origins.get(id(v))for v in m.vertices],"edges":[{"ends":[point(e.a),point(e.b)],"soft":e.soft,"hidden":e.hidden,"curve":e.curve,"layer":e.layer,"raw_incidence":len(e.faces),"incidence":len(set(map(id,e.faces)))}for e in m.edges]}
rows=[]
for name in ["diagonal-protected","ring-island-protected","annular-divisions","diagonal-wires","ring-island-empty","annular-tjunction-invalid"]:
 s=Scene()
 if name.startswith("annular"):
  f=s.mesh.add_face([V(0,0),V(4,0),V(4,4),V(0,4)],hole_loops=[[V(1,1),V(1,3),V(3,3),V(3,1)]]);_push(s,f,3)
 else:_cube(s,History(s),height=3)
 cap=_top(s,3);cap.attrs={"color":[1,0,0],"vendor":9,"ifc":{"id":19,"class":"IfcSlab"}}
 base=next(f for f in s.mesh.faces if all(p.z()==0 for p in f.vertices));s.mesh.remove_face(base)
 if name in ["diagonal-protected","diagonal-wires"]:
  rings=[([V(0,0),V(0,4),V(4,4)],[]),([V(0,0),V(4,4),V(4,0)],[])]
 elif name.startswith("ring-island"):
  rings=[([V(0,0),V(0,4),V(4,4),V(4,0)],[[V(1,1),V(3,1),V(3,3),V(1,3)]]),([V(1,1),V(1,3),V(3,3),V(3,1)],[])]
 elif name=="annular-tjunction-invalid":
  rings=[([V(0,0),V(0,1),V(4,1),V(4,0)],[]),([V(0,3),V(0,4),V(4,4),V(4,3)],[]),([V(0,1),V(0,3),V(1,3),V(1,1)],[]),([V(3,1),V(3,3),V(4,3),V(4,1)],[])]
 else:
  rings=[([V(0,0),V(0,4),V(4,4),V(3,3),V(1,3),V(1,1)],[]),([V(0,0),V(1,1),V(3,1),V(3,3),V(4,4),V(4,0)],[])]
 for i,(outer,holes) in enumerate(rings):
  f=s.mesh.add_face(outer,hole_loops=holes or None);f.attrs={"vendor":71+i,"color":[0,i/4,1],"ifc":{"id":71+i,"class":"IfcSlab"},"layer":"Base"}
 if name=="ring-island-empty":
  next(f for f in s.mesh.faces if f.holes).attrs={}
 if name.startswith("annular"):
  run_stitch(s.mesh,{_key(v.position)for v in s.mesh.vertices},set(),coplanar_merge=False)
  cap=_top(s,3)
 if name=="diagonal-wires":
  for a,b in [(V(7,0),V(8,0)),(V(0,0,3),V(-1,0))]:
   e=s.mesh.add_edge(a,b);e.soft=True;e.hidden=True;e.curve=81;e.layer="Construction"
 for e in s.mesh.edges:
  if e.a.z()==e.b.z()==0 and e.a.x()==e.b.x()==0:e.layer="BaseEdges";e.curve=7
 origins={id(v):i for i,v in enumerate(s.mesh.vertices)};before=snapshot(s.mesh,origins);fid=s.mesh.faces.index(cap);tool=_locked_tool(s,cap,-99);vp=_StubViewport(s);limit=tool.extrusion;tool._commit(vp)
 result=snapshot(s.mesh,origins);vp.history.undo();undo_exact=snapshot(s.mesh,origins)==before;vp.history.redo();redo_exact=snapshot(s.mesh,origins)==result
 rows.append({"source_undo_exact":undo_exact,"source_redo_exact":redo_exact,"name":name,"face":fid,"distance":-99,"ctrl":False,"source":before,"result":snapshot(s.mesh,origins),"clamped_distance":limit,"undo":len(vp.history.undo_stack),"status":vp.last_status})
Path(sys.argv[1]).write_text(json.dumps({"source_pin":"6be29fe437a7cd992d4e079f71720821b25216b9","cases":rows},indent=2)+"\n")
