#!/usr/bin/env python3
"""Actual pinned original import and Viewport, matched native viewport camera."""
import sys,os,json,time,hashlib,subprocess
from pathlib import Path
source,parser,skp,capture,out=map(Path,sys.argv[1:6]);out.mkdir(parents=True,exist_ok=False)
assert subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(parser),'rev-parse','HEAD'],text=True).strip()=='291700bc213655a00461838de71e5f206311e619'
sys.path[:0]=[str(parser/'packages/python/src'),str(source)]
from PySide6.QtCore import QObject,QEvent,Qt
from PySide6.QtGui import QSurfaceFormat,QVector3D
from PySide6.QtWidgets import QApplication
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setDepthBufferSize(24);fmt.setStencilBufferSize(8);fmt.setSamples(4);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from core.scene import Scene
from core.style import Style
from formats.skp import load_skp
from views.viewport import Viewport
record=json.loads((capture/'capture.json').read_text());state=record['records'][0]['state'];camera=state['camera'];scene=Scene();load_skp(scene,skp)
assert [len(scene.dimensions),len(scene.text_labels)]==[state['dimensions'],state['leaders']]
vp=Viewport();vp.setWindowFlags(vp.windowFlags()|Qt.WindowTransparentForInput|Qt.WindowDoesNotAcceptFocus);vp.setFixedSize(800,600);vp.scene=scene;vp.style_override=Style(face_mode='shaded',sky=False,edges=True,profiles=False,background=(1.,1.,1.))
for key,value in camera.items():
 if hasattr(vp.camera,key):setattr(vp.camera,key,QVector3D(*value)if key in['target','up']else value)
input_events=[]
class InputObserver(QObject):
 def eventFilter(self,obj,event):
  if obj is vp and event.type()in[QEvent.MouseButtonPress,QEvent.MouseButtonRelease,QEvent.MouseMove,QEvent.Wheel,QEvent.KeyPress,QEvent.KeyRelease,QEvent.Resize]:
   row=dict(timestamp=time.monotonic(),type=int(event.type()),spontaneous=event.spontaneous(),camera_yaw=vp.camera.yaw,camera_pitch=vp.camera.pitch)
   if hasattr(event,'position'):row['position']=list(event.position().toTuple())
   if hasattr(event,'buttons'):row['buttons']=event.buttons().value
   if hasattr(event,'modifiers'):row['modifiers']=event.modifiers().value
   if hasattr(event,'key'):row['key']=event.key()
   if hasattr(event,'size'):row['size']=[event.size().width(),event.size().height()]
   if len(input_events)<256:input_events.append(row)
   else:row['overflow']=True;input_events[-1]=row
  return False
observer=InputObserver();app.installEventFilter(observer)
initial_navigation={key:repr(getattr(vp,key,None))for key in['_last_pos','_look_drag','nav_mode','_pan_mode','_orbit_pivot']};vp.show();rows=[]
def take(name):
 before=getattr(vp,'_depth_snap',None);started=time.monotonic();vp.update();deadline=started+4
 while time.monotonic()<deadline:
  app.processEvents();time.sleep(.01)
 (out/(name+'-readiness.json')).write_text(json.dumps(dict(timestamp=time.monotonic(),viewport_logical=[vp.width(),vp.height()],framebuffer=list(vp._fb_size()),dpr=vp.devicePixelRatioF(),input_events=input_events,initial_navigation=initial_navigation,camera={key:(list(getattr(vp.camera,key).toTuple())if key in['target','up']else getattr(vp.camera,key))for key in camera if hasattr(vp.camera,key)},gl=dict(vp.gl_info)),indent=2)+'\n')
 assert vp.width()==800 and vp.height()==600
 assert 'NVIDIA GeForce RTX 5090'in vp.gl_info['renderer'],vp.gl_info
 image=vp.grabFramebuffer();assert image.width()==800 and image.height()==600 and not image.isNull();assert image.save(str(out/(name+'.png')))
 matrix=vp.camera.projection_matrix()*vp.camera.view_matrix();inv,ok=matrix.inverted();assert ok
 actual=dict(matrix=list(matrix.data()),inverse=list(inv.data()),eye=list(vp.camera.eye().toTuple()),forward=list(vp.camera.forward().toTuple()))
 actual_camera={key:(list(getattr(vp.camera,key).toTuple())if key in['target','up']else getattr(vp.camera,key))for key in camera if hasattr(vp.camera,key)}
 differences={field:[dict(index=i,expected=a,actual=b)for i,(a,b)in enumerate(zip(expected,actual[field]))if a!=b]for field,expected in state['camera_snapshot'].items()}
 phase=dict(name=name,started=started,captured=time.monotonic(),viewport_logical=[vp.width(),vp.height()],framebuffer=list(vp._fb_size()),dpr=vp.devicePixelRatioF(),camera_expected=camera,camera_actual=actual_camera,camera_snapshot_expected=state['camera_snapshot'],camera_snapshot_actual=actual,differences=differences,input_events=input_events,painted_view=getattr(vp,'_painted_view',None),source_depth_mvp=list(vp._depth_snap[1].data())if vp._depth_snap is not None else None,gl=dict(vp.gl_info))
 (out/(name+'-phase.json')).write_text(json.dumps(phase,indent=2)+'\n')
 for field,diff in differences.items():assert not diff,(name,field,diff)
 snap=vp._depth_snap
 if name!='annotations-off'and state['dimensions']+state['leaders']>0:assert snap is not None and snap[3:5]==(800,600)
 else:assert snap is None
 rows.append(dict(name=name,gl=dict(vp.gl_info),camera_snapshot=actual,depth_snapshot_shape=list(snap[0].shape)if snap is not None else None,image_sha256=hashlib.sha256((out/(name+'.png')).read_bytes()).hexdigest()))
take('annotations-on');dimensions,labels=scene.dimensions,scene.text_labels;scene.dimensions=[];scene.text_labels=[];take('annotations-off');scene.dimensions=dimensions;scene.text_labels=labels;take('annotations-restored')
vp.close();vp.deleteLater();app.processEvents()
(out/'source.json').write_text(json.dumps(dict(source_pin='6be29fe437a7cd992d4e079f71720821b25216b9',parser_pin='291700bc213655a00461838de71e5f206311e619',input_sha256=hashlib.sha256(skp.read_bytes()).hexdigest(),camera=camera,records=rows,scope='actual original Viewport with actual depth capture and annotations; paired native application ROI required'),indent=2)+'\n')
