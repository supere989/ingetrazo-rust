#!/usr/bin/env python3
"""Actual original Rectangle controller on reentered stored drawing frames."""
import os,sys,json
from pathlib import Path
from types import SimpleNamespace as N
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from PySide6.QtCore import QPointF,Qt
from views.viewport import Viewport
from core.group import Group
from core import axes
from tools.rectangle import RectangleTool
from acceptance_context import context
app=QApplication([]);rows=[]
frames=[(r["kind"],r["reentry_drawing_frame"]) for r in json.loads(Path("docs/audits/component-reentry-source.json").read_text())["cases"]]
frames.append(("skewed-mirror",[-2.,0.,0.,0.,.6,3.,0.,0.,0.,.4,1.5,0.,3.,-2.,1.,1.]))
for name,raw in frames:
 for lock in (None,"x","z","world-hover"):
  vp=Viewport();vp.flash_status=lambda *a:None;group=Group(name=name);group.component=True;group.xform=QMatrix4x4();group.axes=QMatrix4x4(*[raw[c*4+r] for r in range(4) for c in range(4)]);group.children=[Group(name="child")];vp.scene.groups=[group]
  vp.begin_group_edit(group);vp.end_one_group_edit();vp.begin_group_edit(group);axes.sync(vp.scene.drawing_frame)
  t=RectangleTool();vp.set_active_tool(t)
  if lock in ("x","z"):assert t.on_key(vp,Qt.Key_Right if lock=="x" else Qt.Key_Up,Qt.NoModifier)
  normal=V(0,0,1) if lock=="world-hover" else axes.axis(lock or "z");start=V(5,6,0) if lock=="world-hover" else axes.origin();u,v=axes.plane_axes(normal);hover=start+u*.7+v*1.2
  if lock=="world-hover":vp._last_work_plane=(V(0,0,0),normal)
  t.on_click(N(viewport=vp,world=start,screen=QPointF(0,0),modifiers=Qt.NoModifier));t.on_hover(N(viewport=vp,world=hover,screen=QPointF(0,0),modifiers=Qt.NoModifier));assert t.on_value(vp,(2.,3.));assert not vp.history.last_error
  f=vp.scene.mesh.faces[0]
  rows.append({"kind":name,"lock":lock,"drawing_frame":raw,"normal":list(normal.toTuple()),"start":list(start.toTuple()),"hover":list(hover.toTuple()),"face_points":[list(p.toTuple()) for p in f.vertices],"history_entries":len(vp.history.undo_stack)})
Path("docs/audits/component-rectangle-axes-source.json").write_text(json.dumps({"provenance":context(),"scope":"Actual original Viewport edit reentry, core.axes.sync, Rectangle key/click/hover/VCB controller; controlled worldpoints, no pointer or rendered proof.","cases":rows},indent=2)+"\n")
print("Original reentered-frame Rectangle twenty controls PASS")
