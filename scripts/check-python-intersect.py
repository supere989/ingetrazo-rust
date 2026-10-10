#!/usr/bin/env python3
"""Original polygon intersection oracle: compare segments in world coordinates."""
import json,os,sys,subprocess,tempfile,math
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core import intersect as X,solids
from core.edits import build_add_edges
from core.history import History
from core.group import Group
from formats import igz
from tests.test_intersect_faces import _box,RED,BLUE
def canonical(segments):return sorted(tuple(sorted(tuple(round(float(c),6) for c in p) for p in segment)) for segment in segments)
def coordinates(v):return [v.x(),v.y(),v.z()]
def main():
 app=QApplication.instance() or QApplication([]);expected={}
 with tempfile.TemporaryDirectory() as tmp:
  directory=Path(tmp)
  for name in ("cross","hole","coplanar","groups"):
   scene=Scene()
   if name=="groups":
    scene.groups=[_box(0,0,0,2,2,2,RED),_box(1,1,1,3,3,3,BLUE)];scene.selection={scene.groups[0]};mode=X.WITH_MODEL
   else:
    size=4 if name=="hole" else 2
    scene.mesh.add_face([V(0,0,0),V(size,0,0),V(size,size,0),V(0,size,0)],[[V(1,1,0),V(1,3,0),V(3,3,0),V(3,1,0)]] if name=="hole" else [])
    if name=="coplanar":scene.mesh.add_face([V(1,1,0),V(3,1,0),V(3,3,0),V(1,3,0)])
    else:
     x=size/2;scene.mesh.add_face([V(x,-1,-1),V(x,size+1,-1),V(x,size+1,1),V(x,-1,1)])
    scene.selection={scene.mesh.faces[0]};mode=X.WITH_CONTEXT
   expected[name]=canonical([[coordinates(a),coordinates(b)] for a,b in X.segments_for(scene,mode)])
   igz.save_scene(scene,directory/(name+"-source.igz"))
  bar=Scene()
  for face in _box(0,0,0,2,2,2,RED).mesh.faces:bar.mesh.add_face(face.vertices)
  bar.groups=[_box(1,-1,1,1.5,3,3,BLUE)];bar.selection=set(bar.mesh.faces);igz.save_scene(bar,directory/"bar-source.igz")
  History(bar).execute(build_add_edges(bar,X.segments_for(bar,X.WITH_MODEL),detect_faces=True))
  subprocess.run(["cargo","run","--locked","-p","ingetrazo-formats","--example","intersect_acceptance_fixture","--",str(directory)],check=True)
  rows=[]
  for name,reference in expected.items():
   actual=canonical(json.loads((directory/(name+"-segments.json")).read_text()));assert actual==reference,(name,actual,reference)
   rows.append({"case":name,"segments":actual})
  rust=Scene();igz.load_into(rust,directory/"bar-result.igz")
  assert solids.solid_report(bar.mesh)[0] and solids.solid_report(rust.mesh)[0],"Intersect must retain closed loose shell"
  a=solids._to_manifold(Group(bar.mesh),solids._Sources());b=solids._to_manifold(Group(rust.mesh),solids._Sources());difference=(a-b).volume()+(b-a).volume()
  assert difference<1e-6 and abs(b.volume()-8)<1e-6,(difference,b.volume())
  vertices=lambda mesh:sorted(set(tuple(round(c,6) for c in coordinates(v.position)) for v in mesh.vertices))
  assert vertices(rust.mesh)==vertices(bar.mesh),"Inserted cut stations must match original mutation"
  assert len(rust.mesh.faces)>6,"Copying the unchanged box cannot satisfy intersection mutation"
  assert len(rust.groups[0].mesh.faces)==len(bar.groups[0].mesh.faces)
  rows.append({"case":"bar-mutation","original_faces":len(bar.mesh.faces),"rust_faces":len(rust.mesh.faces),"native_symmetric_difference_volume":difference,"closed":True})
  assert expected["hole"] and expected["hole"]!=canonical([[[2,0,0],[2,4,0]]]),"Hole interruption must reject an unbroken segment"
 report={"provenance":context(),"passed":True,"negative_controls_detected":1,"cases":rows,"scope":"Exact original world-space intersection segments: crossing squares, hole interruption, coplanar refusal and selected group versus model. Original bar-through-loose-box mutation retains closed native shape and exact cut vertices; healing and context transforms remain separate."}
 Path("docs/audits/python-intersect-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
