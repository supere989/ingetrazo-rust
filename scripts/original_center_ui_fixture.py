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
if mode=='group':
 from core.group import Group
 from core.mesh import Mesh
 from PySide6.QtGui import QMatrix4x4
 group=Group(vp.scene.mesh,name='moving curve group');group.xform=QMatrix4x4();group.component=False
 vp.scene.mesh=Mesh();vp.scene.groups.append(group)
 if os.environ.get('INGETRAZO_GROUP_LINKED')=='1':
  from core.group import copy_group
  group.component=True;vp.scene.groups.append(copy_group(group,V(10,0,0)))
 if os.environ.get('INGETRAZO_GROUP_NESTED')=='1':
  from core.group import copy_group
  outer=Group(name='linked outer context');outer.xform=QMatrix4x4();outer.component=True;outer.children.append(group)
  vp.scene.groups[:]=[outer,copy_group(outer,V(10,0,0))]
 vp.scene.select([group])
vp.set_active_tool(MoveTool() if mode=='group' else LineTool())
if mode=='group':
 # Group array flash_status requires the real main-window statusBar host.
 from PySide6.QtWidgets import QMainWindow
 window=QMainWindow();window.setWindowTitle('IngeTrazo original group Move');window.setCentralWidget(vp);window.resize(1000,700);window.statusBar();window.show()
else:vp.show()
if os.environ.get('INGETRAZO_GROUP_NESTED')=='1':
 vp.begin_group_edit(outer);vp.scene.select([outer.children[0]])
vp.setFocus();directory=Path(sys.argv[1]);timer=QTimer()
# Standalone Viewport does not install the main-window tool/select shortcuts.
# Wire these to the original public actions; geometry/Move/dwell remain original.
shortcuts=[]
for sequence,action in [('Ctrl+A',lambda:vp.scene.select(vp.scene.mesh.edges)),('M',lambda:vp.set_active_tool(MoveTool())),('L',lambda:vp.set_active_tool(LineTool())),('Ctrl+Z',lambda:(vp.history.undo(),vp.notify_scene_changed()))]:
 shortcut=QShortcut(QKeySequence(sequence),vp);shortcut.activated.connect(action);shortcuts.append(shortcut)
def a(p):return[p.x(),p.y(),p.z()]
buffers=[]
def state():
 if vp._value_buffer and (not buffers or buffers[-1]!=vp._value_buffer):buffers.append(vp._value_buffer)
 group_centers=[]
 active_groups=vp.scene.edit_group.children if vp.scene.edit_group is not None else vp.scene.groups
 for group in active_groups:
  local=sum((v.position for v in group.mesh.vertices),V())/max(len(group.mesh.vertices),1)
  group_centers.append(a(group.xform.map(local) if group.xform is not None else local))
 positions=[v.position for v in vp.scene.mesh.vertices[:24]]
 current=V(*group_centers[0]) if mode=='group' and group_centers else sum(positions,V())/len(positions) if positions else center
 if mode=='group' and getattr(vp.active_tool,'_vp_preview',False):current=current+vp._preview_offset
 rim=positions[0] if positions else points[0]
 snap=vp.last_snap;anchor=getattr(vp.active_tool,'start_point',None)
 copy_vertices={id(v):v.position for e in vp.scene.mesh.edges if e.curve is not None and e.curve!=71 for v in(e.v0,e.v1)}
 copy_center=sum(copy_vertices.values(),V())/len(copy_vertices) if copy_vertices else None
 lines=getattr(vp.active_tool,'rubber_band_lines',None)
 copy_probe=vp._world_to_pixel(current+vp.active_tool.hover_point-vp.active_tool.grab+V(0,radius,0)) if getattr(vp.active_tool,'_copy',False) and anchor is not None and vp.active_tool.hover_point is not None else None
 result=dict(context_depth=len(vp.scene._edit_stack),group_mesh_identity=[id(g.mesh) for g in active_groups],buffers=buffers,value_buffer=vp._value_buffer,array_window=getattr(vp.active_tool,"_last",None) is not None,group_centers=group_centers,group_ids=[g.uid for g in active_groups],group_count=len(active_groups),mode=mode,vertices=len(vp.scene.mesh.vertices),unused_vertices=sum(not v.edges for v in vp.scene.mesh.vertices),copy_probe_pixel=copy_probe,copy_center=a(copy_center) if copy_center is not None else None,preview_lines=len(lines()) if callable(lines) else 0,tool=type(vp.active_tool).__name__,curves=[e.curve for e in vp.scene.mesh.edges],time=__import__('time').monotonic(),center=a(current),rim=vp._world_to_pixel(rim),center_pixel=vp._world_to_pixel(current),move_target=vp._world_to_pixel(rim+V(float(os.environ.get("INGETRAZO_MOVE_DISTANCE","1")),0,0)),kind=snap.kind.title().replace('_','') if snap else None,inference_point=a(snap.point) if snap else None,memory=[a(p)for p in vp._encouraged],edges=len(vp.scene.mesh.edges)+sum(len(g.mesh.edges) for g in active_groups),faces=len(vp.scene.mesh.faces),undo=len(vp.history.undo_stack),anchor=a(anchor) if anchor is not None else None)
 ref=vp._valid_center_ref()
 pc=vp._world_to_pixel(ref[0]) if ref is not None else None
 pointer=vp._last_mouse_pos
 hint_radius=vp._center_hint_px(ref,pc) if pc is not None else 0.
 result['center_hint_observation']=dict(radius_px=hint_radius,pointer=[pointer.x(),pointer.y()] if pointer is not None else None,allowed=bool(ref is not None and vp.active_tool is not None and vp.active_tool.uses_snap and not (snap is not None and snap.kind=='center') and pointer is not None and pc is not None and hint_radius>0 and math.hypot(pc[0]-pointer.x(),pc[1]-pointer.y())<=hint_radius))
 temporary=directory/'state.tmp'
 temporary.write_text(json.dumps(result))
 temporary.replace(directory/'state.json')
timer.timeout.connect(state);timer.start(20);app.exec()
