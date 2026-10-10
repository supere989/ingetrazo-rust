#!/usr/bin/env python3
"""Actual source WalkTool.step with controlled analytic visible-face ray fixtures."""
import json,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"ingetrazo"))
from core.camera import OrbitCamera
from tools.walkthrough import WalkTool,set_eye_height
from PySide6.QtCore import QCoreApplication,Qt
from PySide6.QtGui import QVector3D
app=QCoreApplication([])
app.setOrganizationName("IngeTrazoParity");app.setApplicationName("WalkStep")
set_eye_height(1.68)
def vec(v):return [v.x(),v.y(),v.z()]
def state(c):return dict(target=vec(c.target),eye=vec(c.eye()),forward=vec(c.forward()),yaw=c.yaw,pitch=c.pitch,distance=c.distance)
class View:
 def __init__(self,floor,wall):
  self.camera=OrbitCamera();self.camera.look_from(QVector3D(0,0,1.68),QVector3D(0,1,0));self.floor=floor;self.wall=wall
 def ray_distance(self,o,d):
  distances=[]
  if self.floor is not None and d.z()<-1e-9:
   t=(self.floor-o.z())/d.z()
   if t>=0:distances.append(t)
  if self.wall is not None and abs(d.y())>1e-9:
   t=(self.wall-o.y())/d.y()
   if t>=0:distances.append(t)
  return min(distances)if distances else None
rows=[]
for name,dx,dy,dt,run,shift,alt,floor,wall in [
 ("forward",0,150,.033,False,False,False,0,None),
 ("backward",0,-150,.033,False,False,False,0,None),
 ("turn_right",150,0,.033,False,False,False,0,None),
 ("turn_left",-150,0,.033,False,False,False,0,None),
 ("turn_walk",75,150,.033,False,False,False,0,None),
 ("run",0,150,.033,True,False,False,0,None),
 ("dead_zone",3,3,.033,False,False,False,0,None),
 ("edge_dead_zone",4,4,.033,False,False,False,0,None),
 ("clamped",900,900,.033,False,False,False,0,None),
 ("wall",0,150,.1,False,False,False,0,.4),
 ("wall_alt",0,150,.1,False,False,True,0,.4),
 ("strafe",150,0,.1,False,True,False,0,None),
 ("rise",0,150,.1,False,True,False,0,None),
 ("rise_run",0,150,.1,True,True,False,0,None),
 ("stair",0,150,.1,False,False,False,.3,None),
 ("drop",0,150,.1,False,False,False,-.4,None),
 ("open_ground",0,150,.1,False,False,False,None,None)]:
 vp=View(floor,wall);before=state(vp.camera);mods=Qt.NoModifier
 if run:mods|=Qt.ControlModifier
 if shift:mods|=Qt.ShiftModifier
 if alt:mods|=Qt.AltModifier
 WalkTool().step(vp,dx,dy,dt,mods)
 rows.append(dict(name=name,dx=dx,dy=dy,dt=dt,run=run,shift=shift,alt=alt,floor=floor,wall=wall,before=before,after=state(vp.camera)))
Path(sys.argv[1]).write_text(json.dumps(dict(cases=rows,scope="Actual pinned WalkTool.step with analytic floor/wall ray queries; no live mesh/UI acceptance"),indent=2)+"\n")
