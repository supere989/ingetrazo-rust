#!/usr/bin/env python3
import json,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"ingetrazo"))
from core.camera import OrbitCamera
from PySide6.QtGui import QVector3D
def vec(v):return [v.x(),v.y(),v.z()]
def state(c):return dict(target=vec(c.target),eye=vec(c.eye()),forward=vec(c.forward()),yaw=c.yaw,pitch=c.pitch,distance=c.distance)
cases=[]
ops=[("look_from",[[2,3,1.68],[0,1,0]]),("look_from",[[0,0,2],[1,2,-.4]]),("look_from",[[1,-2,3],[0,0,1]]),("look_from",[[1,-2,3],[0,0,-1]]),("look_from",[[2,3,4],[0,0,0]]),("turn",[35,12]),("turn",[-190,-100]),("turn",[720,200]),("move_eye",[[2,-3,.5]]),("move_eye",[[0,0,0]])]
for op,args in ops:
 c=OrbitCamera();before=state(c)
 if op=="look_from":c.look_from(*[QVector3D(*a)for a in args])
 elif op=="move_eye":c.move_eye(QVector3D(*args[0]))
 else:c.turn(*args)
 cases.append(dict(op=op,args=args,before=before,after=state(c)))
Path(sys.argv[1]).write_text(json.dumps(dict(cases=cases,scope="Actual pinned core.camera; numeric records, not rendered/navigation tool parity"),indent=2)+"\n")
