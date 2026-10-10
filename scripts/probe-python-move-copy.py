#!/usr/bin/env python3
"""Operative original Move copy controller, holes/flags and crossing topology."""
import os,sys,json
from pathlib import Path
from types import SimpleNamespace as N
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from PySide6.QtCore import QPointF
from views.viewport import Viewport
from tools.move import MoveTool
from acceptance_context import context
app=QApplication([])
def copy(vp,selection,delta):
 vp.scene.select(selection);t=MoveTool();vp.set_active_tool(t);t.on_click(N(viewport=vp,world=V(0,0,0),screen=QPointF(0,0)));t._copy=True
 assert t.on_value(vp,delta);assert not vp.history.last_error
 return t
vp=Viewport();face=vp.scene.mesh.add_face([V(0,0,0),V(4,0,0),V(4,4,0),V(0,4,0)],hole_loops=[[V(1,1,0),V(1,3,0),V(3,3,0),V(3,1,0)]])
face.attrs.update({"vendor":71,"texture":{"uvw":[1.,0.,0.,2.,0.,1.,0.,3.],"path":"tile.png"}})
for e in vp.scene.mesh.edges:e.soft=True
copy(vp,[face],(10.,0.,0.));m=vp.scene.mesh;assert len(m.faces)==2 and len(m.edges)==16
new=m.faces[-1];assert len(new.holes)==1 and new.attrs["vendor"]==71 and new.attrs["texture"]["uvw"][3]==-8
assert all(e.soft for e in m.edges[:8]) and all(not e.soft for e in m.edges[8:])
rows=[{"name":"holed-painted-face","faces":len(m.faces),"edges":len(m.edges),"copied_holes":len(new.holes),"copied_attrs":new.attrs,"copied_boundary_soft":[e.soft for e in m.edges[8:]]}]
vp=Viewport();edge=vp.scene.mesh.add_edge(V(-1,0,0),V(1,0,0));vp.scene.mesh.add_edge(V(0,1,0),V(0,3,0));copy(vp,[edge],(0.,2.,0.));m=vp.scene.mesh
assert len(m.edges)==5
incident=sum((e.a-V(0,2,0)).length()<1e-6 or(e.b-V(0,2,0)).length()<1e-6 for e in m.edges);assert incident==4
rows.append({"name":"plain-edge-crossing","edges":len(m.edges),"crossing_incidence":incident})
Path("docs/audits/move-copy-source-controls.json").write_text(json.dumps({"provenance":context(),"cases":rows,"scope":"Actual original Move.on_click/on_value + History copy builder; controlled selection/displacement, not pointer or rendered UI. Group/arrays/retyping/cancel remain separate."},indent=2)+"\n")
print("Original Move copy holes/UV/flags and crossing topology PASS")
