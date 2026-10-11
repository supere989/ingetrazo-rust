#!/usr/bin/env python3
"""Pinned actual Line handlers; ordered vertex/endpoint lifetime and command class."""
import os,sys,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QPointF,Qt
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.history import History
from core.edits import build_add_edge
from core.snap import SnapResult
from tools.base import ToolContext
from tools.line import LineTool
from formats.igz import save_scene
app=QApplication.instance() or QApplication([]);out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
def state(s):
 return {"vertices":[list(v.position.toTuple())for v in s.mesh.vertices],"edges":[[s.mesh.vertices.index(e.v0),s.mesh.vertices.index(e.v1)]for e in s.mesh.edges],"faces":[[s.mesh.vertices.index(v)for v in f.loop]for f in s.mesh.faces]}
rows=[]
for case in ['bare-empty','bare-existing','bare-duplicate','bare-off-plane-face','snapshot-plane-face','snapshot-curve','snapshot-crossing','snapshot-closing','midchain-branch']:
 s=Scene();a=[2,0,0];b=[3,0,0]
 if case in ['bare-existing','bare-duplicate']:s.mesh.add_edge(V(0,0,0),V(1,0,0));a=[1,0,0];b=[2,0,0]if case=='bare-existing'else[0,0,0]
 if case in ['bare-off-plane-face','snapshot-plane-face']:
  s.mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)]);a=[2,0,1]if case=='bare-off-plane-face'else[2,0,0];b=[3,0,1]if case=='bare-off-plane-face'else[3,0,0]
 if case=='snapshot-curve':e=s.mesh.add_edge(V(0,0,0),V(1,0,0));e.curve=7;e.soft=True
 if case=='snapshot-crossing':s.mesh.add_edge(V(0,-1,0),V(0,1,0));a=[-1,0,0];b=[1,0,0]
 if case=='snapshot-closing':s.mesh.add_edge(V(0,0,0),V(1,0,0));s.mesh.add_edge(V(1,0,0),V(0,1,0));a=[0,1,0];b=[0,0,0]
 if case=='midchain-branch':a=[0,0,0];b=[2,0,0]
 save_scene(s,out/(case+'.igz'))
 class Viewport:
  scene=s;history=History(s);camera=None
  def update(self):pass
 vp=Viewport();t=LineTool()
 def ctx(p):v=V(*p);return ToolContext(vp,v,QPointF(),Qt.NoModifier,SnapResult(v,'none'))
 before=state(s);command=type(build_add_edge(s,V(*a),V(*b))).__name__
 t.on_click(ctx(a));t.on_click(ctx(b));after=state(s)
 if case=='midchain-branch':t.on_value(vp,(0,3,0));after=state(s)
 if not t.on_undo(vp):vp.history.undo()
 undone=state(s);vp.history.redo();redone=state(s)
 branch=None
 if case=='midchain-branch':
  if not t.on_undo(vp):vp.history.undo()
  t.on_value(vp,(0,-1,0));branch=state(s)
 t.on_cancel(vp)
 rows.append({"case":case,"a":a,"b":b,"class":command,"before":before,"after":after,"undo":undone,"redo":redone,"branch":branch,"cancel":state(s),"anchor":None if t.start_point is None else list(t.start_point.toTuple()),"history":[len(vp.history.undo_stack),len(vp.history.redo_stack)]})
(out/'original.json').write_text(json.dumps(rows,indent=2)+'\n')
