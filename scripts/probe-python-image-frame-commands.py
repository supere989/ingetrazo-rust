#!/usr/bin/env python3
import os,sys,json,math
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.image_plane import ImagePlane
from core.history import MoveImagePlanesCommand,RotateImagePlanesCommand,ScaleImagePlanesCommand,TransformImagePlaneCommand,DeleteImagePlanesCommand
from views.viewport import Viewport
from acceptance_context import context
app=QApplication([]);rows={}
for case in ["move_relative","rotate_z","rotate_arbitrary","scale_world","scale_axes","scale_zero","transform_partial","transform_redo_capture","detached_move"]:
 v=Viewport();v.flash_status=lambda *a:None;im=ImagePlane("scan",V(.125,-1.25,2.5),V(3.25,.125,-.25),V(.375,2.75,.625));v.scene.image_planes.append(im)
 def frame(p):return [[x.x(),x.y(),x.z()]for x in [p.origin,p.u,p.v]]
 def state():return {"live":len(v.scene.image_planes),"frame":frame(im),"undo":len(v.history.undo_stack),"redo":len(v.history.redo_stack)}
 states=[state()]
 if case=="move_relative":cmd=MoveImagePlanesCommand([im],V(.7,1.2,-.3))
 elif case=="rotate_z":cmd=RotateImagePlanesCommand([im],V(.25,-2.5,1.5),V(0,0,-1),90.)
 elif case=="rotate_arbitrary":cmd=RotateImagePlanesCommand([im],V(.25,-2.5,1.5),V(1,2,3),37.5)
 elif case=="scale_world":cmd=ScaleImagePlanesCommand([im],V(.25,-2.5,1.5),(-1.3,2.25,.7))
 elif case=="scale_axes":cmd=ScaleImagePlanesCommand([im],V(.25,-2.5,1.5),(1.3,-.75,2.5),(V(2**-.5,2**-.5,0),V(-2**-.5,2**-.5,0),V(0,0,1)))
 elif case=="scale_zero":cmd=ScaleImagePlanesCommand([im],V(.25,-2.5,1.5),(0,1.5,-2))
 elif case in ["transform_partial","transform_redo_capture"]:cmd=TransformImagePlaneCommand(im,origin=V(.7,1.2,-.3))
 else:
  v.history.execute(DeleteImagePlanesCommand([im]));states.append(state());cmd=MoveImagePlanesCommand([im],V(.7,1.2,-.3))
 v.history.execute(cmd);states.append(state())
 if case=="move_relative":im.origin=im.origin+V(10.125,-.5,3);states.append(state())
 if case=="transform_partial":im.u=V(99,98,97);states.append(state())
 v.history.undo();states.append(state())
 if case=="transform_redo_capture":im.origin=V(7,8,9);states.append(state())
 v.history.redo();states.append(state());v.history.undo();states.append(state());v.history.redo();states.append(state())
 if case=="detached_move":v.history.undo();v.history.undo();states.append(state());v.history.redo();v.history.redo();states.append(state())
 rows[case]=states
Path(sys.argv[1]).write_text(json.dumps({"provenance":context(),"cases":rows},indent=2)+"\n")
