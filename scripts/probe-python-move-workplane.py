#!/usr/bin/env python3
"""Original controller plane recipes with controlled ray and hovered face."""
import sys,os,json,math
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from PySide6.QtCore import QPointF
from views.viewport import Viewport
from tools.move import MoveTool
app=QApplication([]);vp=Viewport();vp.camera.pitch=math.pi/6;vp.camera.yaw=0
face=vp.scene.mesh.add_face([V(-20,-20,0),V(20,-20,0),V(20,20,0),V(-20,20,0)])
move=MoveTool();move.start_point=V(0,0,2);vp.active_tool=move
vp.pick_face_placement=lambda x,y:(face,None)
ray=[V(10,10,10),V(-6,-1,-10).normalized()];vp._pixel_to_ray=lambda x,y:tuple(ray)
def a(v):return[v.x(),v.y(),v.z()]
out={}
for case in ['unselected-face-outside15','unselected-face-within15','arrow-lock','axis-aligned','selected-face']:
 vp.axis_lock='x' if case=='arrow-lock' else None;vp.scene.select([face] if case=='selected-face' else [])
 ray[:]=[V(10,0,10),V(-1,0,-1).normalized()] if case=='axis-aligned' else [V(10,10,10),V(-6,-5 if case=='unselected-face-within15' else -1,-10).normalized()]
 point,normal=vp._current_work_plane((0,0));out[case]=[a(point),a(normal)]
 assert (abs(normal.z())>0.99)==(case=='unselected-face-outside15'),(case,out[case])
vp=Viewport();moving=vp.scene.mesh.add_edge(V(2,3,1),V(4,3,1));vp.scene.mesh.add_edge(V(4,3,1),V(4,4,1));vp.scene.mesh.add_edge(V(4,4,1),V(8,4,1));vp.scene.select([moving]);tool=MoveTool();vp.set_active_tool(tool);tool.on_click(SimpleNamespace(viewport=vp,world=V(2,3,1),screen=QPointF(0,0)))
excluded=tool.snap_excluded()[0];out['excluded-edge-source-indices']=[i for i,e in enumerate(vp.scene.mesh.edges)if id(e)in excluded];assert out['excluded-edge-source-indices']==[0,1]
tool._copy=True;out['copy-exclusion']=tool.snap_excluded();assert out['copy-exclusion'] is None
Path('docs/audits/move-source-workplane-exclusion.json').write_text(json.dumps({'passed':True,'scope':'Actual original _current_work_plane/_hover_face_plane with controlled cursor ray and face placement; Move.on_click/snap_excluded operative source. Controller recipes, not actual GUI picking acceptance.','cases':out},indent=2)+'\n');print('Original Move workplane/exclusion controls PASS')
