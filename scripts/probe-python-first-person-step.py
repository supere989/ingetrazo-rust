#!/usr/bin/env python3
"""Actual source FirstPersonTool.step with controlled analytic visible-face ray fixtures."""
import json,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"ingetrazo"))
from core.camera import OrbitCamera
from tools.walkthrough import FirstPersonTool,set_eye_height
from PySide6.QtCore import QCoreApplication,Qt
from PySide6.QtGui import QVector3D
app=QCoreApplication([])
app.setOrganizationName("IngeTrazoParity");app.setApplicationName("FirstPersonStep")
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
for name,keys,dt,run,alt,floor,wall in [
 ("forward","W",.016,False,False,0,None),
 ("backward","S",.016,False,False,0,None),
 ("right","D",.016,False,False,0,None),
 ("left","A",.016,False,False,0,None),
 ("rise","E",.016,False,False,0,None),
 ("fall","Q",.016,False,False,0,None),
 ("diagonal","WD",.016,False,False,0,None),
 ("three_axes","WDE",.016,False,False,0,None),
 ("cancel","WSADQE",.016,False,False,0,None),
 ("run","W",.016,True,False,0,None),
 ("wall","W",.1,False,False,0,.4),
 ("wall_alt","W",.1,False,True,0,.4),
 ("stair","W",.1,False,False,.3,None),
 ("drop","W",.1,False,False,-.4,None),
 ("open_ground","W",.1,False,False,None,None)]:
 vp=View(floor,wall);before=state(vp.camera);mods=Qt.NoModifier
 if run:mods|=Qt.ShiftModifier
 if alt:mods|=Qt.AltModifier
 FirstPersonTool().step(vp,{getattr(Qt,"Key_"+k) for k in keys},dt,mods)
 rows.append(dict(name=name,axes=[keys.count("W")-keys.count("S"),keys.count("D")-keys.count("A"),keys.count("E")-keys.count("Q")],dt=dt,run=run,alt=alt,floor=floor,wall=wall,before=before,after=state(vp.camera)))
Path(sys.argv[1]).write_text(json.dumps(dict(cases=rows,scope="Actual pinned FirstPersonTool.step with analytic floor/wall ray queries; no live mesh/UI acceptance"),indent=2)+"\n")
