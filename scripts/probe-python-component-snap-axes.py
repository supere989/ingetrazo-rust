#!/usr/bin/env python3
"""Frozen operative original context-axis compute_snap and Shift helpers."""
import os,sys,json
from pathlib import Path
from types import SimpleNamespace as N
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4,QKeyEvent
from PySide6.QtCore import Qt,QEvent
from views.viewport import Viewport
from core import axes
from core.snap import compute_snap
from acceptance_context import context
app=QApplication([]);rows=[]
frames=[(r["kind"],r["reentry_drawing_frame"]) for r in json.loads(Path("docs/audits/component-reentry-source.json").read_text())["cases"]]
frames.append(("skewed-mirror",[-2.,0.,0.,0.,.6,3.,0.,0.,0.,.4,1.5,0.,3.,-2.,1.,1.]))
def px(p):return(1000+100*p.x(),1000-100*p.y())
for name,raw in frames:
 frame=QMatrix4x4(*[raw[c*4+r] for r in range(4) for c in range(4)]);axes.sync(frame)
 vp=Viewport();vp._pixel_to_ray=lambda x,y:(V((x-1000)/100,(1000-y)/100,100),V(0,0,-1));vp._refresh_snap=lambda:None
 start=axes.origin()+V(4,5,0);weak=start+axes.axis("x")*2+axes.axis("y")*.01
 def snap(candidate,anchor=None,axis=None,magnetic=None,shift=None):
  cursor=px(candidate)
  return compute_snap(candidate,cursor,N(edges=[]),px,9,start_point=anchor,axis_lock=axis,magnetic_axis_deg=magnetic,shift_lock_dir=shift,project_onto_line=lambda s,d:vp._project_to_lock_line(s,d,*cursor))
 for mode in ("red-lock","weak-red","magnetic-red","world-origin","context-origin","shift-held","shift-released"):
  anchor=start;axis=None;magnetic=None;shift=None;capture=None
  candidate=weak
  if mode=="red-lock":axis="x";candidate=start+axes.axis("x")*2+axes.axis("y")*.3
  elif mode=="magnetic-red":magnetic=3.
  elif mode=="world-origin":anchor=None;candidate=V(.02,.01,0)
  elif mode=="context-origin":anchor=None;candidate=axes.origin()
  elif mode.startswith("shift"):
   first=snap(weak,start);assert first.kind=="axis_inference" and first.axis=="x"
   vp.active_tool=N(start_point=start);vp.last_snap=first;vp._capture_shift_lock();assert vp._shift_lock is not None
   capture={"kind":first.kind,"axis":first.axis,"point":list(first.point.toTuple()),"source_direction":list(vp._shift_lock[0].toTuple())}
   candidate=start+axes.axis("x")*2+axes.axis("y")*.5
   if mode=="shift-released":vp.keyReleaseEvent(QKeyEvent(QEvent.KeyRelease,Qt.Key_Shift,Qt.NoModifier));assert vp._shift_lock is None
   else:shift=vp._shift_lock[0]
  result=snap(candidate,anchor,axis,magnetic,shift)
  if mode=="world-origin":assert result.kind=="origin"
  if mode=="context-origin" and axes.origin().length()>1.:assert result.kind!="origin"
  rows.append({"name":name+"/"+mode,"frame":raw,"mode":mode,"candidate":list(candidate.toTuple()),"start":list(anchor.toTuple()) if anchor is not None else None,"axis":axis,"magnetic":magnetic,"shift_capture":capture,"original":{"kind":result.kind,"axis":result.axis,"point":list(result.point.toTuple())}})
Path("docs/audits/component-snap-axes-source.json").write_text(json.dumps({"provenance":context(),"scope":"Original core.axes.sync/compute_snap, Viewport._project_to_lock_line/_capture_shift_lock and keyReleaseEvent with controlled orthographic pixel/ray callbacks. Source world origin remains0. Five context frames ×seven locked/angular/magnetic/origin/Shift controls. Not full camera/pointer/occlusion/memory proof.","cases":rows},indent=2)+"\n")
print("Original context screen axes35controls PASS")
