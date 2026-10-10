#!/usr/bin/env python3
"""Frozen original float unprojection and plane arithmetic held-out recipes."""
import json,sys,math
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ingetrazo'))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D
from views.viewport import Viewport
app=QApplication([]);vp=Viewport();vp.resize(800,600)
def vec(v):return [v.x(),v.y(),v.z()]
rows=[]
for view in ['front','right','top','iso']:
 for perspective,two_point in [(False,False),(True,False),(True,True)]:
  for target,distance in [([0,0,0],10),([13,-7,2],3.125)]:
   c=vp.camera;c.target=QVector3D(*target);c.distance=distance;c.aspect=4/3;c.perspective=perspective;c.two_point=two_point;c.set_view(view)
   for x,y in [(430,330),(13,581),(799,1)]:
    origin,direction=vp._pixel_to_ray(x,y);point=vp._ray_plane(origin,direction,QVector3D(),QVector3D(0,0,1))
    rows.append(dict(name=f'{view}-{perspective}-{two_point}-{distance}-{x}-{y}',camera=dict(target=vec(c.target),distance=c.distance,yaw=c.yaw,pitch=c.pitch,up=vec(c.up),fov_deg=c.fov_deg,aspect=c.aspect,znear=c.znear,zfar=c.zfar,perspective=c.perspective,two_point=c.two_point),cursor=[x,y],origin=vec(origin),direction=vec(direction),ground=vec(point)if point is not None else None))
Path(sys.argv[1]).write_text(json.dumps({'cases':rows},indent=2)+'\n')
