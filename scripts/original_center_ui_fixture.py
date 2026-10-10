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
for sequence,action in [('Ctrl+A',lambda:vp.scene.select(vp.scene.mesh.edges)),('M',lambda:vp.set_active_tool(MoveTool())),('L',lambda:vp.set_active_tool(LineTool())),('Ctrl+Z',lambda:(vp.history.undo(),vp.notify_scene_changed()))]:
 shortcut=QShortcut(QKeySequence(sequence),vp);shortcut.activated.connect(action);shortcuts.append(shortcut)
def a(p):return[p.x(),p.y(),p.z()]
def state():
 positions=[v.position for v in vp.scene.mesh.vertices[:24]]
 current=sum(positions,V())/len(positions) if positions else center
 rim=positions[0] if positions else points[0]
 snap=vp.last_snap;anchor=getattr(vp.active_tool,'start_point',None)
 copy_vertices={id(v):v.position for e in vp.scene.mesh.edges if e.curve is not None and e.curve!=71 for v in(e.v0,e.v1)}
 copy_center=sum(copy_vertices.values(),V())/len(copy_vertices) if copy_vertices else None
 lines=getattr(vp.active_tool,'rubber_band_lines',None)
 copy_probe=vp._world_to_pixel(current+vp.active_tool.hover_point-vp.active_tool.grab+V(0,radius,0)) if getattr(vp.active_tool,'_copy',False) and anchor is not None and vp.active_tool.hover_point is not None else None
 result=dict(mode=mode,vertices=len(vp.scene.mesh.vertices),unused_vertices=sum(not v.edges for v in vp.scene.mesh.vertices),copy_probe_pixel=copy_probe,copy_center=a(copy_center) if copy_center is not None else None,preview_lines=len(lines()) if callable(lines) else 0,tool=type(vp.active_tool).__name__,curves=[e.curve for e in vp.scene.mesh.edges],time=__import__('time').monotonic(),center=a(current),rim=vp._world_to_pixel(rim),center_pixel=vp._world_to_pixel(current),move_target=vp._world_to_pixel(rim+V(float(os.environ.get("INGETRAZO_MOVE_DISTANCE","1")),0,0)),kind=snap.kind.title().replace('_','') if snap else None,inference_point=a(snap.point) if snap else None,memory=[a(p)for p in vp._encouraged],edges=len(vp.scene.mesh.edges),faces=len(vp.scene.mesh.faces),undo=len(vp.history.undo_stack),anchor=a(anchor) if anchor is not None else None)
 ref=vp._valid_center_ref()
 pc=vp._world_to_pixel(ref[0]) if ref is not None else None
 pointer=vp._last_mouse_pos
 hint_radius=vp._center_hint_px(ref,pc) if pc is not None else 0.
 result['center_hint_observation']=dict(radius_px=hint_radius,pointer=[pointer.x(),pointer.y()] if pointer is not None else None,allowed=bool(ref is not None and vp.active_tool is not None and vp.active_tool.uses_snap and not (snap is not None and snap.kind=='center') and pointer is not None and pc is not None and hint_radius>0 and math.hypot(pc[0]-pointer.x(),pc[1]-pointer.y())<=hint_radius))
 temporary=directory/'state.tmp'
 temporary.write_text(json.dumps(result))
 temporary.replace(directory/'state.json')
timer.timeout.connect(state);timer.start(20);app.exec()
