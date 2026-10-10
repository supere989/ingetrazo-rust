#!/usr/bin/env python3
"""Actual original Qt/OpenGL Viewport with its own pointer/dwell timer handling."""
import sys,os,json,math
from pathlib import Path
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QTimer,QObject,QEvent
from PySide6.QtGui import QVector3D as V,QShortcut,QKeySequence
source=Path(os.environ.get('INGETRAZO_PYTHON_SOURCE',str(Path(__file__).resolve().parents[1]/'ingetrazo')));sys.path.insert(0,str(source))
from views.viewport import Viewport
from tools.line import LineTool
from tools.move import MoveTool
app=QApplication([])
from views.main_window import MainWindow
from PySide6.QtCore import QPoint
window=MainWindow();window.resize(1000,700);vp=window.viewport
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
  if os.environ.get('INGETRAZO_SOURCE_COMPONENT_CONSTRUCTION')=='command':
   from core.history import MakeComponentOfCommand
   vp.scene.groups[:]=[group];command=MakeComponentOfCommand([],[],[group],name='linked outer context');vp.history.execute(command);outer=command.container;vp.history.undo_stack.clear();vp.history.redo_stack.clear()
  else:
   outer=Group(name='linked outer context');outer.xform=QMatrix4x4();outer.component=True;outer.children.append(group)
  if os.environ.get('INGETRAZO_CONTEXT_EXIT'):outer.xform.translate(1,-1,0)
  vp.scene.groups[:]=[outer,copy_group(outer,V(10,0,0))]
 vp.scene.select([group])
vp.set_active_tool(MoveTool() if mode=='group' else LineTool())
window.show()
if os.environ.get('INGETRAZO_GROUP_NESTED')=='1':
 vp.begin_group_edit(outer);vp.scene.select([outer.children[0]])
vp.setFocus();directory=Path(sys.argv[1]);timer=QTimer()
def pixel(point):
 p=vp._world_to_pixel(point)
 if p is None:return None
 offset=vp.mapTo(window,QPoint(0,0))
 return [p[0]+offset.x(),p[1]+offset.y()]
mouse_events=[]
class MouseObserver(QObject):
 def eventFilter(self,obj,event):
  if event.type()==QEvent.Type.MouseMove:
   mouse_events.append(dict(point=[event.position().x(),event.position().y()],edges=len(vp.scene.mesh.edges),at=__import__('time').monotonic()))
  return False
mouse_observer=MouseObserver(vp);vp.installEventFilter(mouse_observer)
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
 rim=positions[0] if positions else current+V(radius,0,0)
 snap=vp.last_snap;anchor=getattr(vp.active_tool,'start_point',None)
 copy_vertices={id(v):v.position for e in vp.scene.mesh.edges if e.curve is not None and e.curve!=71 for v in(e.v0,e.v1)}
 copy_center=sum(copy_vertices.values(),V())/len(copy_vertices) if copy_vertices else None
 lines=getattr(vp.active_tool,'rubber_band_lines',None)
 copy_probe=pixel(current+vp.active_tool.hover_point-vp.active_tool.grab+V(0,radius,0)) if getattr(vp.active_tool,'_copy',False) and anchor is not None and vp.active_tool.hover_point is not None else None
 root_centers=[]
 for parent in vp.scene.groups:
  child_centers=[]
  for child in parent.children:
   local=sum((v.position for v in child.mesh.vertices),V())/max(len(child.mesh.vertices),1)
   if child.xform is not None:local=child.xform.map(local)
   if parent.xform is not None:local=parent.xform.map(local)
   child_centers.append(a(local))
  root_centers.append(child_centers)
 committed_wire_segments=[]
 rendered_wire_segments=[]
 def projected_wires(group,parent,inherited_preview=False):
  from PySide6.QtGui import QMatrix4x4
  world=parent*group.xform if group.xform is not None else parent
  committed_wire_segments.append(dict(group_id=group.uid,segments=[[pixel(world.map(e.a)),pixel(world.map(e.b))]for e in group.mesh.edges if not e.hidden]))
  preview=inherited_preview or id(group)in(getattr(vp,"_preview_groups",None)or())
  draw=QMatrix4x4(world)
  if preview:
   matrix=getattr(vp,"_preview_matrix",None)
   if matrix is None:
    matrix=QMatrix4x4();matrix.translate(getattr(vp,"_preview_offset",V()))
   draw=matrix*draw
  rendered_wire_segments.append(dict(group_id=group.uid,segments=[[pixel(draw.map(e.a)),pixel(draw.map(e.b))]for e in group.mesh.edges if not e.hidden]))
  for child in group.children:projected_wires(child,world,preview)
 from PySide6.QtGui import QMatrix4x4
 for group in active_groups:projected_wires(group,QMatrix4x4())
 preview_wire_segments=[[pixel(first),pixel(second)]for first,second in lines()] if callable(lines) else []
 result=dict(mouse_events=mouse_events[-32:],scene_version=vp.scene.version,selection_count=len(vp.scene.selection),rendered_wire_segments=rendered_wire_segments,viewport_preview_offset=a(getattr(vp,"_preview_offset",V())),viewport_preview_group_count=len(getattr(vp,"_preview_groups",None)or()),viewport_rect=[vp.mapTo(window,QPoint(0,0)).x(),vp.mapTo(window,QPoint(0,0)).y(),vp.width(),vp.height()],committed_wire_segments=committed_wire_segments,preview_wire_segments=preview_wire_segments,component_construction=os.environ.get("INGETRAZO_SOURCE_COMPONENT_CONSTRUCTION","copy_group_fixture"),active_world_centers=group_centers,root_child_counts=[len(g.children) for g in vp.scene.groups],root_child_world_centers=root_centers,context_depth=len(vp.scene._edit_stack),root_ids=[g.uid for g in vp.scene.groups],group_mesh_identity=[id(g.mesh) for g in active_groups],buffers=buffers,value_buffer=vp._value_buffer,array_window=getattr(vp.active_tool,"_last",None) is not None,group_centers=group_centers,group_ids=[g.uid for g in active_groups],group_count=len(active_groups),mode=mode,vertices=len(vp.scene.mesh.vertices),unused_vertices=sum(not v.edges for v in vp.scene.mesh.vertices),copy_probe_pixel=copy_probe,copy_center=a(copy_center) if copy_center is not None else None,preview_lines=len(lines()) if callable(lines) else 0,tool=type(vp.active_tool).__name__,curves=[e.curve for e in vp.scene.mesh.edges],time=__import__('time').monotonic(),center=a(current),rim=pixel(rim),center_pixel=pixel(current),move_target=pixel(rim+V(float(os.environ.get("INGETRAZO_MOVE_DISTANCE","1")),0,0)),kind=snap.kind.title().replace('_','') if snap else None,inference_point=a(snap.point) if snap else None,memory=[a(p)for p in vp._encouraged],edges=len(vp.scene.mesh.edges)+sum(len(g.mesh.edges) for g in active_groups),faces=len(vp.scene.mesh.faces),undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),main_window_actions=True,anchor=a(anchor) if anchor is not None else None)
 ref=vp._valid_center_ref()
 pc=pixel(ref[0]) if ref is not None else None
 pointer=vp._last_mouse_pos
 hint_radius=vp._center_hint_px(ref,pc) if pc is not None else 0.
 result['action_observation']={'redo_shortcuts':[k.toString() for k in window._redo_action.shortcuts()],'undo_enabled':window._undo_action.isEnabled(),'redo_enabled':window._redo_action.isEnabled()}
 for top_action in window.menuBar().actions():
  if top_action.menu() and any(a is window._redo_action for a in top_action.menu().actions()):
   menu=top_action.menu();c=window.menuBar().actionGeometry(top_action).center();g=window.menuBar().mapTo(window,c);result['edit_menu_pixel']=[g.x(),g.y()]
   if menu.isVisible():
    for label,action in [('undo',window._undo_action),('redo',window._redo_action)]:
     c=menu.actionGeometry(action).center();g=menu.mapToGlobal(c)-window.mapToGlobal(QPoint(0,0));result[label+'_menu_pixel']=[g.x(),g.y()]
 result['center_hint_observation']=dict(valid_reference=ref is not None,radius_px=hint_radius,pointer=[pointer.x(),pointer.y()] if pointer is not None else None,allowed=bool(ref is not None and vp.active_tool is not None and vp.active_tool.uses_snap and not (snap is not None and snap.kind=='center') and pointer is not None and pc is not None and hint_radius>0 and math.hypot(pc[0]-pointer.x(),pc[1]-pointer.y())<=hint_radius))
 temporary=directory/'state.tmp'
 temporary.write_text(json.dumps(result))
 temporary.replace(directory/'state.json')
timer.timeout.connect(state);timer.start(20);app.exec()
