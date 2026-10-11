#!/usr/bin/env python3
"""Actual pinned Qt camera and viewport handlers; unshown widget, ray fallback.
This protocol does not claim retained-frame depth or OS wheel translation.
"""
import os,sys,json,hashlib,subprocess
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtCore import Qt,QPoint,QPointF,QEvent
from PySide6.QtGui import QVector3D,QMouseEvent,QWheelEvent
from PySide6.QtWidgets import QApplication
from core.camera import OrbitCamera
from views.viewport import Viewport
app=QApplication([])
def state(c):return dict(target=[c.target.x(),c.target.y(),c.target.z()],yaw=c.yaw,pitch=c.pitch,distance=c.distance)
def setup(c,initial):
 for k,v in initial.items():setattr(c,k,QVector3D(*v) if k in ("target","up") else v)
def init():return dict(target=[2.25,-1.5,.75],yaw=-.71,pitch=.38,distance=14.,perspective=True,aspect=800/600)
cores=[]
for kind,values in [("orbit",[19.25,-7.5,600]),("orbit",[-80,4000,600]),("zoom",[2,[3,2,1],.05]),("zoom",[-.5,[3,2,1],.05]),("pan",[13,-21,600,37])]:
 c=OrbitCamera();initial=init();setup(c,initial)
 if kind=="orbit":c.orbit_about(QVector3D(3,2,1),*values)
 elif kind=="zoom":c.zoom_to(values[0],QVector3D(*values[1]),values[2])
 else:c.pan(*values)
 cores.append(dict(kind=kind,initial=initial,args=values,result=state(c)))
for perspective in [True,False]:
 c=OrbitCamera();initial=init();initial.update(distance=.02,perspective=perspective);setup(c,initial);c.zoom_to(-1,QVector3D(3,2,1),.4)
 cores.append(dict(kind="zoom",initial=initial,args=[-1,[3,2,1],.4],result=state(c)))
sessions=[]
for name,empty,pan,events in [
 ("face-orbit",False,False,[["press",450,300],["move",480,312],["move",490,317],["release",490,317]]),
 ("bounds-orbit",False,False,[["press",10,10],["move",30,22],["release",30,22]]),
 ("empty-orbit",True,False,[["press",450,300],["move",480,312],["release",480,312]]),
 ("depth-pan",False,True,[["press",450,300],["move",470,315],["release",470,315]]),
 ("wheel-cache",False,False,[["wheel",450,300,1],["wheel",450,300,2],["wheel",450,300,-.5]]),
 ("wheel-empty",True,False,[["wheel",700,500,1],["wheel",700,500,-1]])]:
 w=Viewport();w.resize(800,600);setup(w.camera,init());w._invert_wheel=False;w._invert_orbit_y=False
 points=[[-6,-6,0],[8,-6,0],[8,7,0],[-6,7,0]]
 if not empty:w.scene.mesh.add_face([QVector3D(*p) for p in points]);w.scene.version += 1
 calls=[0];old=w._orbit_pivot_at;oldwheel=w._world_under_cursor
 def orbit(*args):calls[0]+=1;return old(*args)
 def focus(*args):calls[0]+=1;return oldwheel(*args)
 w._orbit_pivot_at=orbit;w._world_under_cursor=focus;rows=[]
 mods=Qt.ShiftModifier if pan else Qt.NoModifier
 for e in events:
  kind,x,y,*rest=e;p=QPointF(x,y)
  if kind=="wheel":ev=QWheelEvent(p,p,QPoint(),QPoint(0,int(rest[0]*120)),Qt.NoButton,mods,Qt.NoScrollPhase,False);w.wheelEvent(ev)
  else:
   t={"press":QEvent.MouseButtonPress,"move":QEvent.MouseMove,"release":QEvent.MouseButtonRelease}[kind]
   button=Qt.NoButton if kind=="move" else Qt.MiddleButton;buttons=Qt.NoButton if kind=="release" else Qt.MiddleButton
   ev=QMouseEvent(t,p,p,button,buttons,mods)
   {"press":w.mousePressEvent,"move":w.mouseMoveEvent,"release":w.mouseReleaseEvent}[kind](ev)
  pivot=getattr(w,"_orbit_pivot",None)
  rows.append(dict(camera=state(w.camera),acquisitions=calls[0],pivot=None if pivot is None else [pivot.x(),pivot.y(),pivot.z()]))
 sessions.append(dict(name=name,empty=empty,pan=pan,points=points,initial=init(),events=events,rows=rows,extent=[w.width(),w.height()]))
 w.close()
out=Path(sys.argv[1]);out.parent.mkdir(parents=True,exist_ok=True)
out.write_text(json.dumps(dict(source_pin=subprocess.check_output(["git","-C","/work/ingetrazo","rev-parse","HEAD"],text=True).strip(),script_sha=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),core=cores,sessions=sessions),indent=2))
# Qt destruction is not a process-close acceptance claim.
os._exit(0)
