#!/usr/bin/env python3
"""Real pinned Viewport pick methods and LayersPanel signals; no picker mocks."""
import os,sys,json,math
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from PySide6.QtCore import Qt
from core import extensions
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.layers import Layer,DEFAULT_LAYER,assign_layer
from formats.igz import save_scene
app=QApplication([]);extensions.plugin_dirs=lambda:[]
from views.main_window import MainWindow
win=MainWindow();vp=win.viewport;vp.resize(800,600)
vp.camera.target=V(0,0,0);vp.camera.pitch=math.pi/2;vp.camera.yaw=-math.pi/2;vp.camera.distance=20;vp.camera.perspective=False
vp.camera.aspect=vp.width()/vp.height()
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
rows=[]
def quad(m,z=0):return m.add_face([V(-2,-2,z),V(2,-2,z),V(2,2,z),V(-2,2,z)])
def vec(v):return list(v.toTuple())
def snapshot(case,phase,p):
 s=vp.scene;xy=vp._world_to_pixel(p);ray=vp._pixel_to_ray(*xy)
 f=vp.pick_face(*xy);e=vp.pick_edge(*xy);g=vp.pick_group(*xy)
 v=vp.pick_vertex(*vp._world_to_pixel(V(-2,-2,0)))
 name=f"{case}-{phase}";save_scene(s,out/(name+".igz"))
 rows.append({"name":name,"ray":[vec(x)for x in ray],"point":vec(p),"face":None if f is None else s.faces.index(f),"edge":None if e is None else s.edges.index(e),"group":None if g is None else s.groups.index(g),"vertex":None if v is None else vec(v),"selection":sorted(["face" if x in s.faces else "edge" if x in s.edges else "group" for x in s.selection]),"visible_faces":[s.entity_visible(f)for f in s.faces]})
for case in ["face-default","face-empty","face-named","face-unknown","face-rear","edge-default","edge-empty","edge-named","group-default","group-named","group-face-named","child-named","parent-named","line-group-named"]:
 s=Scene();vp.scene=s;s.layers.append(Layer("Named"));target="Layer 0" if "default" in case or "empty" in case else "Named";p=V(0,0,0)
 if case.startswith("face"):
  f=quad(s.mesh);f.attrs["layer"]="" if "empty" in case else "Missing" if "unknown" in case else target
  if "default" in case:f.attrs.pop("layer")
  if "rear" in case:
   f.attrs["layer"]="Named";quad(s.mesh,-2)
 elif case.startswith("edge"):
  e=s.mesh.add_edge(V(-2,-2,0),V(2,-2,0));e.layer="" if "empty" in case else None if "default" in case else target;p=V(0,-2,0)
 else:
  g=Group(Mesh(),name="root")
  if "line" in case:g.mesh.add_edge(V(-2,-2,0),V(2,-2,0));p=V(0,-2,0)
  else:quad(g.mesh)
  if case in ["group-default","group-named","line-group-named"]:g.layer=None if "default" in case else target
  elif case=="group-face-named":g.mesh.faces[0].attrs["layer"]="Named"
  else:
   child=g;g=Group(Mesh(),name="parent");g.children=[child]
   (child if case=="child-named" else g).layer="Named"
   g.xform=QMatrix4x4();g.xform.translate(5,1,2);g.xform.rotate(30,0,0,1);child.xform=g.xform.inverted()[0]
  s.groups.append(g)
 s.version+=1;win.tray.layers.refresh();panel=win.tray.layers
 item=next(panel.tree.topLevelItem(i)for i in range(panel.tree.topLevelItemCount())if panel.tree.topLevelItem(i).text(0)==target)
 snapshot(case,"open",p)
 s.selection.update(s.faces+s.edges+s.groups)
 item.setCheckState(2,Qt.Checked);snapshot(case,"locked",p)
 item.setCheckState(2,Qt.Unchecked);item.setCheckState(1,Qt.Unchecked);snapshot(case,"hidden",p)
 item.setCheckState(1,Qt.Checked);snapshot(case,"restored",p)
(out/"original.json").write_text(json.dumps({"rows":rows},indent=2))
print(json.dumps({"cases":len(rows),"output":str(out)}));win._saved_version=vp.scene.version;win.close()
