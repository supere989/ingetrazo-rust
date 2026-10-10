#!/usr/bin/env python3
import os,sys,json,tempfile,subprocess
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group,world_mesh
from core.dimension import Dimension
from formats import igz
def wire():
 m=Mesh();m.add_edge(V(0,0,0),V(1,0,0));return m
def transform(x,y,z):
 m=QMatrix4x4();m.translate(x,y,z);return m
def tuplev(p):return tuple(round(x,6) for x in (p.x(),p.y(),p.z()))
def check_dim(scene,a,b):
 d=scene.dimensions[0];assert tuplev(d.a)==a and tuplev(d.b)==b,(tuplev(d.a),tuplev(d.b),a,b)
def main():
 subprocess.run([sys.executable,"scripts/probe-python-dimension-entry-undo.py"],check=True)
 app=QApplication.instance() or QApplication([]);rows=[]
 with tempfile.TemporaryDirectory() as tmp:
  directory=Path(tmp);s=Scene();s.mesh=wire();s.dimensions=[Dimension(V(0,0,0),V(1,0,0),V(0,1,0))];igz.save_scene(s,directory/"source.igz")
  s=Scene();g=Group(wire(),"Translated part");g.xform=transform(10,20,30);s.groups=[g];s.dimensions=[Dimension(V(10,20,30),V(11,20,30),V(0,1,0))];igz.save_scene(s,directory/"context-source.igz")
  s=Scene();leaf=Group(wire(),"Leaf");leaf.xform=transform(1,2,3);middle=Group(name="Middle");middle.xform=transform(10,20,30);middle.children=[leaf];parent=Group(name="Parent");parent.xform=transform(100,200,300);parent.children=[middle];s.groups=[parent];s.dimensions=[Dimension(V(111,222,333),V(112,222,333),V(0,1,0))];igz.save_scene(s,directory/"outliner-source.igz")
  subprocess.run(["cargo","run","--locked","-p","ingetrazo-gui","--example","dimension_anchor_acceptance_fixture","--",str(directory)],check=True)
  for name,a,b in [("moved",(0,0,0),(3,0,0)),("scaled",(10,20,30),(16,20,30)),("exploded",(10,20,30),(16,20,30)),("deleted",(10,20,30),(16,20,30)),("context",(10,20,30),(11,20,30)),("outliner",(111,222,333),(112,222,333))]:
   reopened=Scene();igz.load_into(reopened,directory/(name+".igz"));check_dim(reopened,a,b)
   if name=="moved":
    vertex=next(v for v in reopened.mesh.vertices if tuplev(v.position)==b);reopened.mesh.place_vertex(vertex,V(7,0,0));check_dim(reopened,a,(7,0,0))
   if name=="scaled":
    vertex=next(v for v in reopened.groups[0].mesh.vertices if tuplev(v.position)==(3,0,0));reopened.groups[0].mesh.place_vertex(vertex,V(5,0,0));check_dim(reopened,a,(20,20,30))
   if name=="context":
    assert len(reopened.dimensions)==2;d=reopened.dimensions[1];assert tuplev(d.a)==a and tuplev(d.b)==(14,20,30) and tuplev(d.offset)==(0,2,0) and d.text=="Local <>"
   if name=="outliner":
    leaf=reopened.groups[0].children[0].children[0];assert leaf.name=="Edited leaf"
    source=Scene();igz.load_into(source,directory/"outliner-source.igz");assert leaf.uid==source.groups[0].children[0].children[0].uid
   rows.append({"case":name,"original_runtime_endpoints":[a,b],"passed":True})
 report={"provenance":context(),"passed":True,"cases":rows,"scope":"Stable anchors refreshed at GUI frame boundaries: movement/group/scale/explode/deletion with source command-specific mesh shareback preserving annotations and replaced-vertex endpoint fallback; original native reopen and original live vertex/group endpoint rebinding. Translated-context world dimension creation and three-level outliner actual selection/edit/rename+UID retention/undo. Nested nonuniform/reflected world-frame measurement and rendered inspector remain separate."}
 Path("docs/audits/python-dimension-anchor-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
