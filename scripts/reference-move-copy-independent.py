import os,sys,json
from pathlib import Path
from types import SimpleNamespace as N
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from PySide6.QtCore import QPointF,Qt
from views.viewport import Viewport
from tools.move import MoveTool
from core.group import Group,iter_placements
a=QApplication([])
def copy(vp,sel,d):
 vp.resize(800,600);vp.show();
 for _ in range(10):a.processEvents()
 vp.scene.select(sel);t=MoveTool();vp.set_active_tool(t);t.on_click(N(viewport=vp,world=V(0,0,0),screen=QPointF(0,0)));t._copy=True;assert t.on_value(vp,d);assert not vp.history.last_error
def point(v):return [v.x(),v.y(),v.z()]
rows=[]
vp=Viewport();e1=vp.scene.mesh.add_edge(V(0,0,0),V(1,0,0));e2=vp.scene.mesh.add_edge(V(1,0,0),V(1,1,0));e1.soft=True;e2.soft=True;e1.curve=91;e2.curve=91
copy(vp,[e1,e2],(5.,0.,0.));edges=vp.scene.mesh.edges
rows.append({"name":"selected-curve","edges":len(edges),"source_curve":edges[0].curve,"copied_curve":edges[-1].curve,"copied_link":edges[-1].curve==edges[-2].curve,"fresh_curve":edges[-1].curve!=edges[0].curve,"copied_soft":[e.soft for e in edges[-2:]],"copied_attrs":[{} for e in edges[-2:]]})
for component in [False,True]:
 vp=Viewport();g=Group(name="parent");g.mesh.add_edge(V(0,0,0),V(1,0,0));g.component=component;g.xform=QMatrix4x4();g.xform.translate(V(2,3,0));g.material={"color":[1,0,0]};g.ext={"vendor":{"path":"do-not-remap"}}
 child=Group(name="child");child.mesh.add_edge(V(0,0,0),V(0,1,0));child.component=False;g.children=[child];vp.scene.groups.append(g);copy(vp,[g],(5.,0.,0.));c=vp.scene.groups[-1]
 pts=[]
 for pg,xf in iter_placements(c):
  for e in pg.mesh.edges:pts.extend([point(xf.map(e.a) if xf is not None else e.a),point(xf.map(e.b) if xf is not None else e.b)])
 rows.append({"name":"component" if component else "ordinary","world_points":pts,"material":c.material,"ext":c.ext,"fresh_root":c.uid!=g.uid,"fresh_child":c.children[0].uid!=child.uid,"component":c.component,"shared_component_mesh":c.mesh is g.mesh if component else None})
vp=Viewport();vp.flash_status=lambda _:None;t=MoveTool();vp.set_active_tool(t);states=[]
for mods in [Qt.NoModifier,Qt.ControlModifier,Qt.NoModifier]:
 assert t.on_key(vp,Qt.Key_Control,mods);states.append(t._copy)
t._reset();rows.append({"name":"control-toggle","press_states":states,"reset_clears":not t._copy})
Path(sys.argv[1]).write_text(json.dumps(rows,indent=2))
print(json.dumps(rows))
