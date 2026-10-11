#!/usr/bin/env python3
"""Pinned shape handlers through actual MainWindow activation and Viewport History.
Controlled world points/pixel projection are explicit recipe inputs, not desktop proof.
"""
import os,sys,json,hashlib,argparse,subprocess
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from PySide6.QtCore import QPointF,Qt
from views.main_window import MainWindow
from tools.base import ToolContext
from core.edits import build_add_edges
from core import axes
PIN="6be29fe437a7cd992d4e079f71720821b25216b9"
def vec(p):return list(p.toTuple())
def capture(vp,t):
 m=vp.scene.mesh;verts=list(m.vertices);vi={id(v):i for i,v in enumerate(verts)};curves={}
 def curve(c):
  if c is None:return None
  if c not in curves:curves[c]=len(curves)
  return curves[c]
 return {"vertices":[vec(v.position) for v in verts],"edges":[{"vertices":[vi[id(e.v0)],vi[id(e.v1)]],"soft":e.soft,"hidden":e.hidden,"layer":e.layer,"curve":curve(e.curve)} for e in m.edges],"faces":[{"vertices":[vi[id(v)] for v in f.loop],"holes":[[vi[id(v)] for v in h] for h in f.hole_loops],"attrs":f.attrs,"interior":f.interior} for f in m.faces],"preview":[[vec(a),vec(b)] for a,b in t.rubber_band_lines()],"caption":t.vcb_caption() if hasattr(t,"vcb_caption") else t.vcb_label,"start":vec(t.start_point) if t.start_point is not None else None,"history":[len(vp.history.undo_stack),len(vp.history.redo_stack)],"last_error":vp.history.last_error,"preview_color":getattr(t,"wireframe_color",None)}
def event(op,*args):return [op,*args]
def recipes():
 out=[]
 def case(name,tool,ops,**extra):out.append(dict(name=name,tool=tool,operations=ops,**extra))
 for tool in ("circle","polygon"):
  case(tool+"-default-click",tool,[event("click",[0,0,0]),event("move",[2,0,0]),event("click",[2,0,0]),event("undo"),event("redo")])
  case(tool+"-radius-hover-direction",tool,[event("click",[1,1,0]),event("move",[3,4,0]),event("value",5),event("undo"),event("redo")])
  case(tool+"-sides-and-cancel-repeat",tool,[event("value",7),event("click",[0,0,0]),event("move",[1,2,0]),event("segments",9),event("cancel"),event("click",[0,0,0]),event("move",[2,1,0]),event("value",2),event("click",[5,0,0]),event("move",[6,1,0]),event("click",[6,1,0]),event("undo"),event("redo")])
  case(tool+"-vertical-view-plane",tool,[event("plane",[0,0,0],[0,1,0]),event("click",[0,0,0]),event("move",[2,0,1]),event("value",3)])
  case(tool+"-oblique-view-plane",tool,[event("plane",[0,0,0],[1,1,1]),event("click",[0,0,0]),event("move",[1,-1,0]),event("click",[1,-1,0])])
  case(tool+"-arrow-lock",tool,[event("key","Right"),event("click",[0,0,0]),event("move",[0,2,1]),event("value",2)])
  case(tool+"-off-plane-snap",tool,[event("plane",[4,0,0],[-1,0,0]),event("click",[4,.1,.9]),event("move",[.27,.1,3.31]),event("click",[.27,.1,3.31])])
  case(tool+"-true-diagonal-project",tool,[event("plane",[4,0,0],[-1,0,0]),event("click",[4,.1,.9]),event("move",[.27,.6,3.31]),event("click",[.27,.6,3.31])])
  case(tool+"-invalid-value-preservation",tool,[event("click",[0,0,0]),event("value",2),event("move",[1,0,0]),event("value",0),event("segments",2),event("value",2)])
 case("circle-crossing-existing-face","circle",[event("click",[2,0,0]),event("move",[2,2,0]),event("click",[2,2,0]),event("undo"),event("redo")],square=True)
 case("two-point-bulge-click","arc",[event("click",[0,0,0]),event("click",[4,0,0]),event("move",[2,1,0]),event("click",[2,1,0]),event("undo"),event("redo")])
 case("two-point-bulge-vcb","arc",[event("click",[0,0,0]),event("click",[4,0,0]),event("move",[2,-1,0]),event("value",.5),event("segments",8),event("undo"),event("redo")])
 case("two-point-radius-vcb","arc",[event("click",[0,0,0]),event("click",[4,0,0]),event("move",[2,1,0]),event("radius",1),event("radius",3),event("undo"),event("redo")])
 case("two-point-tangent-edge","arc",[event("click",[1.5,0,0]),event("move",[1,1,0]),event("cancel")],square=True)
 case("two-point-corner-repeat","arc",[event("click",[1.5,0,0]),event("move",[2,.5,0]),event("click",[2,.5,0]),event("double",[2,.5,0]),event("segments",8),event("double",[0,2,0]),event("undo"),event("redo")],square=True)
 case("three-point-click","arc3",[event("click",[0,0,0]),event("click",[2,2,0]),event("move",[4,0,0]),event("value",2),event("click",[4,0,0]),event("undo"),event("redo")])
 case("three-point-collinear","arc3",[event("click",[0,0,0]),event("click",[1,0,0]),event("move",[2,0,0]),event("click",[2,0,0])])
 case("three-point-off-plane","arc3",[event("plane",[4,0,0],[-1,0,0]),event("click",[4,.1,.9]),event("click",[2,.1,3.9]),event("move",[.27,.1,3.31]),event("click",[.27,.1,3.31])])
 for tool in ("center_arc","pie"):
  case(tool+"-click",tool,[event("click",[1,1,0]),event("click",[3,1,0]),event("move",[1,3,0]),event("click",[1,3,0]),event("undo"),event("redo")])
  case(tool+"-angle-vcb-negative-hover",tool,[event("click",[0,0,0]),event("click",[2,0,0]),event("move",[0,-2,0]),event("value",90),event("undo"),event("redo")])
  case(tool+"-snapped-arm",tool,[event("plane",[4,0,0],[-1,0,0]),event("click",[4,.1,.9]),event("move",[.27,.1,3.31]),event("click",[.27,.1,3.31]),event("move",[4,.1,5]),event("value",90)])
 for tool in ("arc","arc3","center_arc","pie"):
  case(tool+"-arrow-cancel-repeat",tool,[event("key","Left"),event("click",[0,0,0]),event("move",[1,0,2]),event("cancel"),event("click",[0,0,0]),event("click",[2,0,0]),event("move",[1,1,0]),event("click",[1,1,0])])
 for name,ops in [
 ("diagonal-click",[event("click",[0,0,0]),event("click",[3,3,0]),event("move",[2,4,0]),event("click",[2,4,0])]),
 ("base-length-vcb",[event("click",[0,0,0]),event("move",[3,4,0]),event("value",10),event("move",[6,10,0]),event("value",2)]),
 ("base-and-width-angle-vcb",[event("click",[0,0,0]),event("move",[1,0,0]),event("value",[2,90]),event("move",[-1,2,1]),event("value",[3,90])]),
 ("width-keeps-hover-angle",[event("click",[0,0,0]),event("click",[2,0,0]),event("move",[2,-1,1]),event("value",3)]),
 ("zero-width-refusal",[event("click",[0,0,0]),event("click",[2,0,0]),event("move",[2,0,0]),event("click",[2,0,0]),event("value",0),event("move",[2,2,0]),event("value",2)]),
 ("shift-base",[event("click",[0,0,0]),event("move",[3,1,0],True),event("move",[1,4,0],True),event("click",[1,4,0]),event("move",[2,2,0]),event("value",2)]),
 ("shift-width",[event("click",[0,0,0]),event("click",[2,0,0]),event("move",[2,3,0]),event("move",[2,3,0],True),event("move",[2,1,5],True),event("value",2)]),
 ("protractor-near-far",[event("click",[0,0,0]),event("move",[.39931776,.30090751,0]),event("move",[2.3959067,1.805445,0]),event("cancel")]),
 ("arrow-lock-wall",[event("key","Right"),event("click",[0,0,0]),event("click",[0,2,0]),event("move",[0,2,3]),event("click",[0,2,3])]),
 ("cancel-repeat",[event("click",[0,0,0]),event("move",[1,2,0]),event("cancel"),event("click",[0,0,0]),event("value",[2,0]),event("move",[2,2,0]),event("value",2)])]:
  case("rotated-"+name,"rotated_rect",ops+[event("undo"),event("redo")],point_snaps=True)
 for tool in ("circle","polygon"):
  case(tool+"-reference-edge-lock",tool,[event("edge",[0,0,0],[1,1,1]),event("key","Down"),event("click",[1,1,1]),event("move",[2,0,1]),event("value",2)])
  case(tool+"-nested-drawing-axes",tool,[event("context"),event("click",[0,0,0]),event("move",[0,2,1]),event("value",2),event("undo"),event("redo")])
 case("rotated-width-ray-reading","rotated_rect",[event("click",[0,0,0]),event("click",[2,0,0]),event("ray",[5,-4,3],[2,-1,1]),event("move",[6.5,-5.5,0]),event("value",2)])
 case("rotated-width-real-point-snap","rotated_rect",[event("click",[0,0,0]),event("click",[2,0,0]),event("ray",[5,-4,3],[2,-1,1]),event("move",[2,3,0]),event("value",2)],point_snaps=True)
 return out

def addendum():
 out=[]
 def case(name,tool,ops,**extra):out.append(dict(name=name,tool=tool,operations=ops,**extra))
 for tool in ("circle","polygon"):
  case(tool+"-rounded-side-count-and-cursor",tool,[event("move",[1,1,0]),event("value",2.6),event("move",[1,1,0]),event("key","Right"),event("move",[0,1,1]),event("click",[0,1,1]),event("move",[0,2,1]),event("value",1),event("undo"),event("redo")])
 case("two-point-delivered-corner-repeat","arc",[event("click",[1.5,0,0]),event("move",[2,.5,0]),event("click",[2,.5,0]),event("double",[2,.5,0]),event("segments",8),event("click",[0,2,0]),event("double",[0,2,0]),event("undo"),event("redo")],square=True)
 case("two-point-radius-without-hover","arc",[event("click",[0,0,0]),event("click",[4,0,0]),event("radius",3),event("undo"),event("redo")])
 case("two-point-half-snap-color","arc",[event("click",[0,0,0]),event("click",[2,0,0]),event("move",[1,.96,0]),event("click",[1,.96,0]),event("undo"),event("redo")])
 case("two-point-fillet-radius","arc",[event("click",[1.5,0,0]),event("move",[2,.5,0]),event("click",[2,.5,0]),event("move",[1.85,.15,0]),event("value",.25),event("undo"),event("redo")],square=True)
 case("two-point-matching-radius-opposite-hover","arc",[event("click",[1.5,0,0]),event("move",[2,.5,0]),event("click",[2,.5,0]),event("move",[1,1,0]),event("radius",.5),event("undo"),event("redo")],square=True)
 return out

def remaining():
 out=[]
 def case(name,tool,ops,**extra):out.append(dict(name=name,tool=tool,operations=ops,**extra))
 case("two-point-curve-end-continuation","arc",[event("click",[0,0,0]),event("click",[2,0,0]),event("move",[1,1,0]),event("click",[1,1,0]),event("click",[2,0,0]),event("click",[3,-1,0]),event("move",[2.25,-.75,0]),event("click",[2.25,-.75,0]),event("undo"),event("redo")])
 for tool in ("arc","arc3","center_arc","pie","rotated_rect"):
  case(tool+"-nested-context",tool,[event("context"),event("click",[0,0,0]),event("click",[0,2,0]),event("move",[0,1,2]),event("click",[0,1,2]),event("undo"),event("redo")],point_snaps=True)
 return out

def main():
 args=argparse.ArgumentParser();args.add_argument("output");args.add_argument("--addendum",action="store_true");args.add_argument("--remaining",action="store_true");a=args.parse_args();pin=subprocess.check_output(["git","-C","/work/ingetrazo","rev-parse","HEAD"],text=True).strip();assert pin==PIN,pin
 app=QApplication.instance() or QApplication([]);win=MainWindow();vp=win.viewport;vp.resize(1000,700);vp.camera.set_aspect(1000,700);vp.flash_status=lambda *a,**k:None
 rows=[]
 for recipe in (remaining() if a.remaining else addendum() if a.addendum else recipes()):
  vp.scene.groups.clear();vp.scene.mesh.clear();vp.history.undo_stack.clear();vp.history.redo_stack.clear();axes.sync(None);vp._last_work_plane=(V(0,0,0),V(0,0,1));vp.axis_lock=None;vp._hover_edge=None
  if recipe.get("square"):
   pts=[V(0,0,0),V(2,0,0),V(2,2,0),V(0,2,0)];vp.history.execute(build_add_edges(vp.scene,list(zip(pts,pts[1:]+pts[:1]))));vp.history.undo_stack.clear()
  from tools.arc import ArcTool
  ArcTool.last_fillet_d=None
  cls=type(win._tools[recipe["tool"]]);win._tools[recipe["tool"]]=cls()
  win._activate_tool(recipe["tool"]);t=vp.active_tool
  # Explicit observation projection: same 100px/m plan law as source protractor tests.
  vp._world_to_pixel=lambda p:(p.x()*100.,-p.y()*100.-p.z()*100.)
  vp._pixel_to_ray=lambda x,y:(V(x/100.,-y/100.,10),V(0,0,-1))
  states=[dict(phase="activated",state=capture(vp,t))]
  for i,op in enumerate(recipe["operations"]):
   kind=op[0];accepted=None
   if kind in ("click","move","double"):
    p=V(*op[1]);mods=Qt.ShiftModifier if len(op)>2 and op[2] else Qt.NoModifier
    from core.snap import SnapResult
    snap=SnapResult(p,"endpoint") if recipe.get("point_snaps") else None
    ctx=ToolContext(viewport=vp,world=p,screen=QPointF(*vp._world_to_pixel(p)),modifiers=mods,snap=snap)
    getattr(t,{"click":"on_click","move":"on_hover","double":"on_double_click"}[kind])(ctx)
   elif kind=="plane":vp._last_work_plane=(V(*op[1]),V(*op[2]))
   elif kind=="edge":
    e=vp.scene.mesh.add_edge(V(*op[1]),V(*op[2]));vp._hover_edge=e
   elif kind=="ray":
    origin,aim=V(*op[1]),V(*op[2]);direction=(aim-origin).normalized();vp._pixel_to_ray=lambda x,y,o=origin,d=direction:(o,d)
   elif kind=="context":
    from core.group import Group
    from PySide6.QtGui import QMatrix4x4
    g=Group(name="turned");g.axes=QMatrix4x4();g.axes.rotate(90,V(0,1,0));vp.scene.groups=[g];vp.begin_group_edit(g);axes.sync(vp.scene.drawing_frame)
   elif kind=="key":accepted=t.on_key(vp,getattr(Qt,"Key_"+op[1]),Qt.NoModifier)
   elif kind=="value":accepted=t.on_value(vp,tuple(op[1]) if isinstance(op[1],list) else op[1])
   elif kind in ("segments","radius"):accepted=getattr(t,"on_"+kind+"_value")(vp,op[1])
   elif kind=="cancel":t.on_cancel(vp)
   elif kind in ("undo","redo"):getattr(vp.history,kind)()
   else:raise ValueError(op)
   states.append(dict(phase=str(i)+"-"+kind,accepted=accepted,state=capture(vp,t)))
  rows.append(dict(recipe=recipe,activation=type(t).__name__,states=states))
  Path(a.output+".partial").write_text(json.dumps({"source_pin":PIN,"cases":rows},indent=2)+"\n")
  if getattr(vp.scene,"_edit_stack",None):
   while vp.scene._edit_stack:vp.end_one_group_edit()
 source_files=["tools/circle.py","tools/arc.py","tools/rotated_rectangle.py","tools/base.py","core/edits.py","core/history.py","core/axes.py"]
 result={"source_pin":PIN,"source_sha256":{f:hashlib.sha256((Path('/work/ingetrazo')/f).read_bytes()).hexdigest() for f in source_files},"scope":"Actual MainWindow activation, original tools and Viewport History; explicit world/pixel recipes, no rendered or desktop-input claim.","cases":rows}
 Path(a.output).write_text(json.dumps(result,indent=2)+"\n");print("captured",len(rows),"shape workflows")
if __name__=="__main__":main()
