import os,sys,json
from pathlib import Path
from types import SimpleNamespace as N
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from PySide6.QtCore import QPointF,Qt
from views.viewport import Viewport
from tools.move import MoveTool
from core.group import Group,iter_placements
a=QApplication([])
def copy(vp,sel,d):
 vp.resize(800,600);vp.show();
 for _ in range(10):a.processEvents()
 vp.scene.select(sel);t=MoveTool();vp.set_active_tool(t);t.on_click(N(viewport=vp,world=V(0,0,0),screen=QPointF(0,0)));t._copy=True;assert t.on_value(vp,d);assert not vp.history.last_error;return t
def point(v):return [v.x(),v.y(),v.z()]
rows=[]
window=QMainWindow();vp=Viewport();window.setCentralWidget(vp);window.resize(800,600);window.show();face=vp.scene.mesh.add_face([V(0,0,0),V(4,0,0),V(4,4,0),V(0,4,0)],hole_loops=[[V(1,1,0),V(1,3,0),V(3,3,0),V(3,1,0)]])
face.attrs.update({"vendor":71,"texture":{"uvw":[1.,0.,0.,2.,0.,1.,0.,3.],"path":"tile.png"}})
e1=vp.scene.mesh.add_edge(V(0,10,0),V(1,10,0));e2=vp.scene.mesh.add_edge(V(1,10,0),V(1,11,0));e1.soft=e2.soft=True;e1.curve=e2.curve=91
g=Group(name="private");g.component=False;g.xform=QMatrix4x4();g.xform.translate(V(2,3,0));g.mesh.add_edge(V(0,0,0),V(1,0,0));vp.scene.groups=[g]
t=copy(vp,[face,e1,e2,g],(18.,0.,0.))
def snapshot(text):
 faces=sorted((min(p.x()for p in f.vertices),len(f.holes),f.attrs["vendor"],f.attrs["texture"]["uvw"][3])for f in vp.scene.mesh.faces)
 curve_ids=[e.curve for e in vp.scene.mesh.edges if e.curve is not None];ids=set(curve_ids)
 roots=sorted(pg.xform.map(V(0,0,0)).x()for pg in vp.scene.groups)
 return {"text":text,"faces":faces,"edges":len(vp.scene.mesh.edges),"edge_segments":sorted([sorted([point(e.a),point(e.b)])for e in vp.scene.mesh.edges]),"curve_edges":len(curve_ids),"curve_groups":len(ids),"source_curve_retained":91 in ids,"pair_links":all(curve_ids.count(i)==2 for i in ids),"groups_x":roots,"undo":len(vp.history.undo_stack)}
for text,count,mode in [("3x",3,"x"),("/3",3,"/"),("2x",2,"x")]:
 assert t.on_array_value(vp,count,mode);rows.append(snapshot(text))
vp.history.undo();undo=snapshot("undo");vp.history.redo();redo=snapshot("redo")
Path(sys.argv[1]).write_text(json.dumps({"states":rows,"undo":undo,"redo":redo},indent=2));print(json.dumps(rows))
