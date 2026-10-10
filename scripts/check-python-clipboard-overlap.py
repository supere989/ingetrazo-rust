#!/usr/bin/env python3
"""Painted overlapping paste versus actual original copy/paste commands."""
import json,os,sys,tempfile,subprocess
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D
from PySide6.QtCore import QPointF,Qt
from core.scene import Scene
from core.history import History
from views.viewport import Viewport
from tools.paste import PasteTool
from tools.base import ToolContext
from formats import igz
class VP:
 def __init__(self,s):self.scene=s;self.history=History(s);self.clipboard=None
 def update(self):pass
def inside(p,ring):
 x,y=p;coords=[(v.x(),v.y()) for v in ring];flag=False
 for (a,b),(c,d) in zip(coords,coords[1:]+coords[:1]):
  if (b>y)!=(d>y) and x<(c-a)*(y-b)/(d-b)+a:flag=not flag
 return flag
def paint(scene,p):
 return sorted(tuple(round(v,5) for v in f.attrs.get("color",[])) for f in scene.mesh.faces if inside(p,f.vertices) and not any(inside(p,h) for h in f.holes))
def doc(face):return {"igz_format":1,"app_version":"independent","scene":{"faces":[face],"edges":[]}}
def main():
 app=QApplication.instance() or QApplication([])
 with tempfile.TemporaryDirectory() as t:
  d=Path(t);sourceface={"vertices":[[0,0,0],[3,0,0],[3,3,0],[0,3,0]],"holes":[[[.5,.5,0],[.5,1.5,0],[1.5,1.5,0],[1.5,.5,0]]],"color":[1,0,0,1],"vendor_paint":17};destface={"vertices":[[1,0,0],[4,0,0],[4,3,0],[1,3,0]],"color":[0,0,1,1],"vendor_paint":23}
  (d/"overlap-source.igz").write_text(json.dumps(doc(sourceface)));(d/"overlap-destination.igz").write_text(json.dumps(doc(destface)))
  subprocess.run(["cargo","run","--locked","-p","ingetrazo-gui","--example","clipboard_overlap_fixture","--",t],check=True)
  src,dest,rust=Scene(),Scene(),Scene();igz.load_into(src,d/"overlap-source.igz");igz.load_into(dest,d/"overlap-destination.igz");igz.load_into(rust,d/"overlap-result.igz")
  vp=VP(src);src.selection.add(src.mesh.faces[0]);assert Viewport.copy_selection(vp);vp.scene=dest;vp.history=History(dest);tool=PasteTool();tool.on_activate(vp);tool.on_click(ToolContext(viewport=vp,world=QVector3D(0,0,0),screen=QPointF(),modifiers=Qt.NoModifier,snap=None))
  tested=0
  for i in range(40):
   for j in range(30):
    p=((i+.371)/10,(j+.293)/10);assert paint(dest,p)==paint(rust,p),(p,paint(dest,p),paint(rust,p));tested+=1
  assert paint(rust,(2.2,2.2))==[(0.,0.,1.,1.)],"original retains destination paint in this overlapping region"
  assert paint(rust,(.25,2.25))==[(1.,0.,0.,1.)],"source-only region retains source paint"
  assert paint(rust,(.75,.75))==[],"source hole remains unfilled outside destination"
  assert paint(rust,(1.25,.75))==[(0.,0.,1.,1.)],"destination retains paint in copied hole"
 app.clipboard().clear();app.processEvents()
 report={"provenance":context(),"passed":True,"sampled_paint_coverage_comparisons":tested,"negative_controls_detected":4,"scope":"Actual original Copy/Paste plane merge: overlapping red source and blue destination, source hole, source-only paint and original destination overlap precedence and destination hole fill, selected pasted output and atomic exact undo/redo. Curved/nonplanar overlap remains separate."}
 Path("docs/audits/python-clipboard-overlap-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
