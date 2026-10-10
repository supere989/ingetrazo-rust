#!/usr/bin/env python3
"""Actual original Qt/OpenGL Viewport with its own pointer/dwell timer handling."""
import sys,os,json,math
from pathlib import Path
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QTimer
from PySide6.QtGui import QVector3D as V,QShortcut,QKeySequence
source=Path(os.environ.get('INGETRAZO_PYTHON_SOURCE',str(Path(__file__).resolve().parents[1]/'ingetrazo')));sys.path.insert(0,str(source))
from views.viewport import Viewport
from tools.line import LineTool
from tools.move import MoveTool
app=QApplication([]);vp=Viewport();vp.resize(1000,700);vp.setWindowTitle('IngeTrazo original center tracking')
center=V(2,3,1);mode=os.environ.get('INGETRAZO_CENTER_MODE','curve');radius=.01 if mode=='tiny' else 1.2

points=[center+V(radius*math.cos(i*math.tau/24),radius*math.sin(i*math.tau/24),0) for i in range(24)]
for i,point in enumerate(points):vp.scene.mesh.add_edge(point,points[(i+1)%24]).curve=71
if mode=='malformed':vp.scene.mesh.add_edge(points[0],center+V(3,0,0)).curve=71
vp.camera.target=V(2,3,0);vp.camera.distance=10;vp.camera.pitch=math.pi/6;vp.camera.yaw=-math.pi/2;vp.camera.perspective=True
vp.set_active_tool(LineTool());vp.show();vp.setFocus();directory=Path(sys.argv[1]);timer=QTimer()
# Standalone Viewport does not install the main-window tool/select shortcuts.
# Wire these to the original public actions; geometry/Move/dwell remain original.
shortcuts=[]
for sequence,action in [('Ctrl+A',lambda:vp.scene.select(vp.scene.mesh.edges)),('M',lambda:vp.set_active_tool(MoveTool())),('L',lambda:vp.set_active_tool(LineTool()))]:
 shortcut=QShortcut(QKeySequence(sequence),vp);shortcut.activated.connect(action);shortcuts.append(shortcut)
def a(p):return[p.x(),p.y(),p.z()]
def state():
 positions=[v.position for v in vp.scene.mesh.vertices[:24]]
 current=sum(positions,V())/len(positions) if positions else center
 rim=positions[0] if positions else points[0]
 snap=vp.last_snap;anchor=getattr(vp.active_tool,'start_point',None)
 result=dict(mode=mode,tool=type(vp.active_tool).__name__,curves=[e.curve for e in vp.scene.mesh.edges],time=__import__('time').monotonic(),center=a(current),rim=vp._world_to_pixel(rim),center_pixel=vp._world_to_pixel(current),move_target=vp._world_to_pixel(rim+V(float(os.environ.get("INGETRAZO_MOVE_DISTANCE","1")),0,0)),kind=snap.kind.title().replace('_','') if snap else None,inference_point=a(snap.point) if snap else None,memory=[a(p)for p in vp._encouraged],edges=len(vp.scene.mesh.edges),faces=len(vp.scene.mesh.faces),undo=len(vp.history.undo_stack),anchor=a(anchor) if anchor is not None else None)
 directory.joinpath('state.json').write_text(json.dumps(result))
timer.timeout.connect(state);timer.start(20);app.exec()
