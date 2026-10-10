#!/usr/bin/env python3
"""Actual original Move controller + viewport/snap functions; no GUI claim."""
import sys,os,json,math,subprocess,hashlib
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from PySide6.QtCore import QPointF
from views.viewport import Viewport
from tools.move import MoveTool
from core.snap import compute_snap
app=QApplication([])
def a(p):return[p.x(),p.y(),p.z()]
inputs=[];expected=[]
for name,delta,yaw,copy,lock,mixed in [('short-self-excluded',(.05,0,0),-math.pi/2,False,None,False),('copy-self-kept',(.05,0,0),-math.pi/2,True,None,False),('magnetic15',(1,.2,0),-math.pi/2,False,None,False),('outside15',(1,.6,0),-math.pi/2,False,None,False),('oblique-screen-y',(0,1,0),math.pi/4,False,None,False),('explicit-arrow',(1,.5,0),math.pi/4,False,'z',False),('independent-near-endpoint',(.02,0,0),-math.pi/2,False,None,True)]:
 vp=Viewport();vp.resize(800,800);start=V(2,3,1);vp.camera.target=V(start);vp.camera.distance=10;vp.camera.pitch=math.pi/6;vp.camera.yaw=yaw;vp.camera.perspective=True;vp.camera.aspect=1
 edge=vp.scene.mesh.add_edge(start,start+V(4,0,0));vp.scene.mesh.add_edge(start+V(4,0,0),start+V(4,1,0))
 if mixed:vp.scene.mesh.add_edge(start+V(0,.05,0),start+V(0,1,0))
 vp.scene.select([edge]);tool=MoveTool();vp.set_active_tool(tool);tool.on_click(SimpleNamespace(viewport=vp,world=V(start),screen=QPointF(*vp._world_to_pixel(start))));tool._copy=copy;vp.axis_lock=lock
 desired=start+V(*delta);pixel=vp._world_to_pixel(desired);candidate=vp._world_from_pixel(*pixel);normal=vp._current_work_plane(pixel)[1];ray=vp._pixel_to_ray(*pixel)
 source=compute_snap(candidate,pixel,vp._snap_scene(*pixel),vp._world_to_pixel,9,project_onto_line=lambda s,d:vp._project_to_lock_line(s,d,*pixel),start_point=start,axis_lock=lock,magnetic_axis_deg=tool.magnetic_axis_deg,screen_axis_px=tool.screen_axis_px)
 excluded=tool.snap_excluded();ids=sorted(i for i,e in enumerate(vp.scene.mesh.edges)if excluded is not None and id(e)in excluded[0]) if excluded is not None else None
 inputs.append(dict(name=name,edges=[[a(e.a),a(e.b)]for e in vp.scene.mesh.edges],copy=copy,start=a(start),candidate=a(candidate),normal=a(normal),ray=[a(p)for p in ray],pixel=pixel,lock=lock,camera=dict(target=a(vp.camera.target),pitch=vp.camera.pitch,yaw=yaw)))
 expected.append(dict(name=name,kind={'axis':'AxisLock','axis_inference':'AxisInference','on_face':'OnFace','endpoint':'Endpoint','none':'None'}.get(source.kind,source.kind),point=a(source.point),axis=source.axis,excluded=ids))
subprocess.run(['cargo','build','--locked','-p','ingetrazo-tools','--example','move_snap_acceptance_fixture'],check=True)
binary=Path(os.environ.get('CARGO_TARGET_DIR','target'))/'debug/examples/move_snap_acceptance_fixture';actual=json.loads(subprocess.check_output([str(binary.resolve())],input=json.dumps(inputs).encode()))
assert len(inputs)==len(expected)==len(actual)==7, "Incomplete Move controller protocol"
failures=[]
for source,native in zip(expected,actual):
 if source['name']!=native['name'] or source['kind']!=native['kind'] or source['axis']!=native['axis'] or source['excluded']!=native['excluded'] or sum((a-b)**2 for a,b in zip(source['point'],native['point']))**.5>1e-5:failures.append(dict(source=source,native=native))
report=dict(passed=not failures,scope='Original actual Move.on_click, snap_excluded, Viewport._current_work_plane/_world_from_pixel/_snap_scene and compute_snap; Rust real Move targets/policy inference. Controller recipes, not GUI events.',source_root=str(Path('ingetrazo').resolve()),fixture_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),native_executable_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),inputs=inputs,original=expected,native=actual,failures=failures)
Path('docs/audits/move-inference-source-evidence.json').write_text(json.dumps(report,indent=2)+'\n');print('Move controller source/native',len(inputs)-len(failures),'/',len(inputs));assert not failures,failures
