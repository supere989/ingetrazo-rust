import sys,os,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.history import History
from tests.test_pushpull_slanted import _gable_house,_wedge,_roof,_gable_face,_wedge_faces,_roof_rect
from tests.test_pushpull_against_wall import _step_with_square
from tests.test_fuzz_engine import _draw_rect
from tests.test_pushpull_ux import _push
app=QApplication.instance()or QApplication([])
def mesh(m):
 return [{"vertices":[[p.x(),p.y(),p.z()]for p in f.vertices],"holes":[[[p.x(),p.y(),p.z()]for p in h]for h in f.holes],"attrs":f.attrs,"interior":bool(getattr(f,"interior",False))}for f in m.faces]
cases=[]
def run(name,s,f,d,ctrl=False):
 before=mesh(s.mesh);idx=s.mesh.faces.index(f);vp=_push(s,f,d,ctrl);cases.append({"name":name,"source":before,"face":idx,"distance":d,"ctrl":ctrl,"result":mesh(s.mesh),"status":vp.last_status})
for d in [.3,-.2]:
 s,h=_gable_house();run("roof"+str(d),s,_roof(s),d)
s,h=_gable_house();run("gable-extend",s,_gable_face(s),1.)
s,h=_gable_house();n,rect=_roof_rect(s);_draw_rect(s,h,rect,[]);f=next(f for f in s.mesh.faces if len(f.vertices)==4 and not f.holes and f.area()<1.1 and abs(V.dotProduct(f.normal().normalized(),n))>.999);run("slanted-pocket",s,f,-.2)
for i in range(4):
 for d in [.6,-.4]:
  s,h=_wedge();run("wedge"+str(i)+"-"+str(d),s,_wedge_faces(s)[i],d)
for d in [.5,1.,1.5,-.5]:
 s,f=_step_with_square();run("step"+str(d),s,f,d)
for d in [1.,-1.]:
 s=Scene();outer=[V(0,0,0),V(4,0,0),V(4,4,0),V(0,4,0)];hole=[V(1,1,0),V(1,3,0),V(3,3,0),V(3,1,0)];s.mesh.add_face(outer,hole_loops=[hole]);f=s.mesh.add_face(list(reversed(hole)));run("open-pocket"+str(d),s,f,d)
for ctrl in [False,True]:
 s=Scene();f=s.mesh.add_face([V(0,0,0),V(2,0,0),V(2,1,0),V(0,1,0)]);f.attrs={"vendor":71,"color":[1,0,0],"opacity":.7,"ifc":{"id":4,"class":"IfcSlab","name":"painted"},"texture":{"path":"wall.png","uvw":[1,0,0,0,0,1,0,0],"tile":2}}
 run("painted-free-ctrl"+str(ctrl),s,f,1.,ctrl)
for d in [.3,-.2]:
 s,h=_gable_house();run("ctrl-roof"+str(d),s,_roof(s),d,True)
for d in [.5,-.5]:
 s,f=_step_with_square();run("ctrl-step"+str(d),s,f,d,True)
for d in [1.,-1.]:
 s=Scene();f=s.mesh.add_face([V(0,0,0),V(4,0,0),V(4,4,0),V(0,4,0)],hole_loops=[[V(1,1,0),V(1,3,0),V(3,3,0),V(3,1,0)]]);run("ctrl-holed-free"+str(d),s,f,d,True)
Path("docs/audits/pushpull-gate2/source.json").write_text(json.dumps({"cases":cases},indent=2)+"\n")
print(len(cases),"original cases frozen")
