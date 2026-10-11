import os,sys,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.history import History
from tests.test_pushpull_ux import _cube,_top,_locked_tool,_StubViewport,_push
from core import solids
from core.group import Group
app=QApplication.instance()or QApplication([])
def faces(m):return [{"vertices":[[p.x(),p.y(),p.z()]for p in f.vertices],"holes":[[[p.x(),p.y(),p.z()]for p in h]for h in f.holes],"attrs":f.attrs,"interior":bool(getattr(f,"interior",False))}for f in m.faces]
rows=[]
for name,d,holes,paint in [("zero",0,False,False),("subminimum",.00002,False,False),("exact-minimum",.0002,False,False),("collapse",-3,False,False),("past-collapse",-99,False,False),("painted-collapse",-3,False,True),("holed-collapse",-1,True,True),("holed-near-endpoint",-.999,True,False)]:
 s=Scene()
 if holes:
  f=s.mesh.add_face([V(0,0,0),V(4,0,0),V(4,4,0),V(0,4,0)],hole_loops=[[V(1,1,0),V(1,3,0),V(3,3,0),V(3,1,0)]]);_push(s,f,1);top=_top(s,1)
 else:_cube(s,History(s),height=3);top=_top(s,3)
 if paint:top.attrs={"vendor":71,"color":[1,0,0],"ifc":{"id":1,"class":"IfcSlab"}}
 before=faces(s.mesh);fid=s.mesh.faces.index(top);tool=_locked_tool(s,top,d);vp=_StubViewport(s)
 clamped=tool.extrusion
 if abs(d)>=.0002:tool._commit(vp)
 rows.append({"name":name,"source":before,"face":fid,"distance":d,"ctrl":False,"clamped_distance":clamped,"result":faces(s.mesh),"undo":len(vp.history.undo_stack),"status":vp.last_status,"vertices":len(s.mesh.vertices),"edges":len(s.mesh.edges)})
from core.edits import build_add_edges
from core.history import AddFaceCommand
for name,initial,requested in [("bump-flush",1,-1),("corner-through",0,-3),("mouth-through",0,-3)]:
 s=Scene();h=History(s);_cube(s,h,height=3)
 points=[V(1,1,3),V(3,1,3),V(3,3,3),V(1,3,3)] if name!="corner-through" else [V(0,0,3),V(2,0,3),V(2,2,3),V(0,2,3)]
 h.execute(build_add_edges(s,[(points[i],points[(i+1)%4])for i in range(4)],detect_faces=False,extra=[AddFaceCommand(points)]))
 f=next(f for f in s.mesh.faces if len(f.vertices)==4 and all(abs(p.z()-3)<1e-6 for p in f.vertices) and f.area()<5)
 f.attrs={"vendor":71,"color":[1,0,0]}
 if initial:_push(s,f,initial);f=next(f for f in s.mesh.faces if len(f.vertices)==4 and all(abs(p.z()-4)<1e-6 for p in f.vertices))
 before=faces(s.mesh);fid=s.mesh.faces.index(f);tool=_locked_tool(s,f,requested);vp=_StubViewport(s);clamped=tool.extrusion;tool._commit(vp)
 rows.append({"name":name,"source":before,"face":fid,"distance":requested,"ctrl":False,"clamped_distance":clamped,"result":faces(s.mesh),"undo":len(vp.history.undo_stack),"status":vp.last_status,"vertices":len(s.mesh.vertices),"edges":len(s.mesh.edges)})
s=Scene();_cube(s,History(s),height=3);f=_top(s,3);f.attrs={"vendor":71,"color":[1,0,0],"ifc":{"id":1,"class":"IfcSlab"}};tool=_locked_tool(s,f,-3);tool._commit(_StubViewport(s));before=faces(s.mesh);tool=_locked_tool(s,s.mesh.faces[0],-3);vp=_StubViewport(s);clamped=tool.extrusion;tool._commit(vp);rows.append({"name":"repeat-after-collapse","source":before,"face":0,"distance":-3,"ctrl":False,"clamped_distance":clamped,"result":faces(s.mesh),"undo":len(vp.history.undo_stack),"status":vp.last_status,"vertices":len(s.mesh.vertices),"edges":len(s.mesh.edges)})
limits=[]
for name in ["wider-punchable-sheet","annular-support"]:
 s=Scene()
 if name=="wider-punchable-sheet":
  _cube(s,History(s),height=3);f=_top(s,3)
  s.mesh.add_face([V(-1,-1,1.5),V(5,-1,1.5),V(5,5,1.5),V(-1,5,1.5)])
 else:
  f=s.mesh.add_face([V(0,0,0),V(4,0,0),V(4,4,0),V(0,4,0)],hole_loops=[[V(1,1,0),V(1,3,0),V(3,3,0),V(3,1,0)]]);_push(s,f,1);f=_top(s,1)
 tool=_locked_tool(s,f,-99)
 limits.append({"name":name,"source":faces(s.mesh),"face":s.mesh.faces.index(f),"clamped_distance":tool.extrusion,"limit":tool._limit_in})
Path(sys.argv[1]).write_text(json.dumps({"source_pin":"6be29fe437a7cd992d4e079f71720821b25216b9","cases":rows,"limit_controls":limits},indent=2)+"\n")
