#!/usr/bin/env python3
"""Missing P07 authored-guide/camera sessions only; original product stays untouched."""
import argparse,hashlib,json,math,os,subprocess,sys
from pathlib import Path
from types import SimpleNamespace as N
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import Qt,QPointF
from PySide6.QtGui import QVector3D as V
from views.main_window import MainWindow
from core.guide import Guide
from core.snap import compute_snap,SnapResult
from tools.base import ToolContext
from core import axes
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
def vec(p):return [p.x(),p.y(),p.z()]
def state(vp):
 p=lambda v:[round(c,5)for c in vec(v)]
 return {'guides':[g.to_dict() for g in vp.scene.guides],'vertices':sorted(p(v.position)for v in vp.scene.mesh.vertices),'edges':sorted(sorted([p(e.a),p(e.b)])for e in vp.scene.mesh.edges),'history':[len(vp.history.undo_stack),len(vp.history.redo_stack)]}
def main():
 ap=argparse.ArgumentParser();ap.add_argument('output',type=Path);a=ap.parse_args();assert not a.output.exists();a.output.mkdir(parents=True)
 assert subprocess.check_output(['git','-C','/work/ingetrazo','rev-parse','HEAD'],text=True).strip()==PIN
 app=QApplication([]);win=MainWindow();vp=win.viewport;vp.resize(1000,800);vp.flash_status=lambda *args,**kw:None;rows=[]
 configs=[('perspective',20.,-37.,31.,False,True),('close',.17,-23.,38.,False,True),('two-point',12.3,-51.,24.,True,True),('steep-fallback',7.1,-33.,83.,True,True),('parallel',8.3,-21.,42.,False,False)]
 center=V(3,2,1)
 for name,distance,yaw,pitch,two,perspective in configs:
  cam=vp.camera;cam.target=V(center);cam.distance=distance;cam.yaw=math.radians(yaw);cam.pitch=math.radians(pitch);cam.aspect=1.25;cam.perspective=perspective;cam.two_point=two;cam.fov_deg=47.3
  config={k:getattr(cam,k) for k in ['distance','yaw','pitch','aspect','perspective','two_point','fov_deg','znear','zfar']};config.update(target=vec(cam.target),up=vec(cam.up))
  for label,offset in [('cross',0.),('point-inside9',8.999),('point-exact9',9.),('point-outside9',9.001),('line-inside14',13.999),('line-exact14',14.),('line-outside14',14.001)]:
   vp.scene.mesh.clear();vp.scene.groups.clear();vp.scene.guides.clear();vp.history.undo_stack.clear();vp.history.redo_stack.clear();axes.sync(None)
   inputs=[];states=[]
   # Two actual Protractor transactions with exact snapped endpoint targets.
   for end in [center+V(1,1,0),center+V(-1,1,0)]:
    win._tools['protractor']=type(win._tools['protractor'])();win._activate_tool('protractor');t=vp.active_tool;t.work_plane=(V(center),V(0,0,1))
    pts=[center,center+V(1,0,0),end];inputs.append([vec(x)for x in pts])
    for point in pts:t.on_click(ToolContext(vp,V(point),QPointF(0,0),Qt.NoModifier,SnapResult(V(point),'endpoint')))
   authored=[g.to_dict()for g in vp.scene.guides];states.append({'phase':'guides','state':state(vp)})
   if label.startswith('point'):vp.scene.guides=[Guide(center)]
   elif label.startswith('line'):vp.scene.guides=vp.scene.guides[:1]
   win._tools['line']=type(win._tools['line'])();win._activate_tool('line');vp._last_work_plane=(V(center),V(0,0,1));pixel=vp._world_to_pixel(center)
   direction=(1.,0.)
   if label.startswith('line'):
    other=vp._world_to_pixel(center+vp.scene.guides[0].direction*(distance*.01));dx=other[0]-pixel[0];dy=other[1]-pixel[1];length=math.hypot(dx,dy);direction=(-dy/length,dx/length)
   cursor=[pixel[i]+offset*direction[i]for i in range(2)];candidate=vp._world_from_pixel(*cursor)
   hit=compute_snap(candidate,tuple(cursor),vp._snap_scene(*cursor),vp._world_to_pixel,threshold_px=9,edge_threshold_px=14,project_onto_line=lambda s,d:vp._project_to_lock_line(s,d,*cursor),work_plane_normal=V(0,0,1),is_occluded=vp._is_occluded)
   spans=[None if not g.is_line else [vec(x)for x in vp._guide_snap_segment(g)]for g in vp.scene.guides]
   # Exact original production _build_ctx rounds event pixels to integers.
   actual=vp._build_ctx(N(position=lambda:QPointF(*pixel),modifiers=lambda:Qt.NoModifier))
   if label=='cross':
    vp.active_tool.on_click(actual);states.append({'phase':'anchor','state':state(vp)});vp.active_tool.on_value(vp,(1.,0.,0.));states.append({'phase':'line','state':state(vp)});vp.active_tool.on_cancel(vp);vp.history.undo();states.append({'phase':'undo','state':state(vp)});vp.history.redo();states.append({'phase':'redo','state':state(vp)})
   rows.append({'name':name+'-'+label,'camera':config,'producer_points':inputs,'authored':authored,'guides':[g.to_dict()for g in vp.scene.guides],'matrix':list((cam.projection_matrix()*cam.view_matrix()).data()),'view':list(cam.view_matrix().data()),'projection':list(cam.projection_matrix().data()),'cursor':cursor,'candidate':vec(candidate),'source':{'kind':hit.kind,'point':vec(hit.point)},'segments':spans,'production_context':{'kind':actual.snap.kind,'point':vec(actual.world)},'states':states})
 files=['core/camera.py','core/guide.py','core/snap.py','views/viewport.py','tools/protractor.py','tools/line.py'];(a.output/'source.json').write_text(json.dumps({'source_pin':PIN,'source_files':{f:hashlib.sha256((Path('/work/ingetrazo')/f).read_bytes()).hexdigest()for f in files},'cases':rows,'scope':'Five actual camera modes, authored Protractor guides, variable-w clipping, production snap callbacks and Line/history; exact logical pixel limits plus integer event _build_ctx. No full Protractor or rendered pixel claim.'},indent=2)+'\n');print(json.dumps({'cases':len(rows),'workflow_states':sum(len(r['states'])for r in rows)}));win._saved_version=vp.scene.version;win.close()
if __name__=='__main__':main()
