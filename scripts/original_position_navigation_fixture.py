#!/usr/bin/env python3
import json,sys,ctypes,ctypes.util
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ingetrazo'))
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QTimer,QPoint,QEvent,QObject
from PySide6.QtGui import QVector3D
from views.main_window import MainWindow
from tools.walkthrough import set_eye_height,eye_height
app=QApplication([]);set_eye_height(1.68);window=MainWindow();window.setWindowTitle('IngeTrazo Position navigation proof');window.resize(1000,700);vp=window.viewport;vp.camera.distance=10;vp.camera.target=QVector3D();vp.camera.perspective=False;vp.camera.set_view('top');vp.scene.mesh.add_edge(QVector3D(2,2,0),QVector3D(2.5,2,0));window._activate_tool('position_camera');window.show();vp.setFocus();directory=Path(sys.argv[1]);frames=0;pointer=None;gl_metadata=None
class Observer(QObject):
 def eventFilter(self,obj,event):
  global pointer
  if event.type()==QEvent.Type.MouseMove:
   pos=vp.mapTo(window,event.position().toPoint());pointer=[pos.x(),pos.y()]
  return False
observer=Observer(vp);vp.installEventFilter(observer)
def vec(v):return[v.x(),v.y(),v.z()]
def state():
 global frames,gl_metadata
 if gl_metadata is None and vp.isValid():
  vp.makeCurrent();gl=ctypes.CDLL(ctypes.util.find_library('GL'));gl.glGetString.argtypes=[ctypes.c_uint];gl.glGetString.restype=ctypes.c_char_p
  gl_metadata={name:gl.glGetString(token).decode()for name,token in [('vendor',0x1f00),('renderer',0x1f01),('version',0x1f02)]};vp.doneCurrent()
 frames+=1;p=vp.mapTo(window,QPoint(0,0));lines=vp.active_tool.rubber_band_lines();pixels=[]
 for line in lines:
  pair=[]
  for point in line:
   pixel=vp._world_to_pixel(point);pair.append([pixel[0]+p.x(),pixel[1]+p.y()]if pixel else None)
  pixels.append(pair)
 mode=type(vp.active_tool).__name__;endpoint=vp._world_to_pixel(QVector3D(2,2,0));data=dict(eye_height=eye_height(),value_buffer=vp._value_buffer,measurement=window._vcb_value.text(),measurement_label=window._vcb_name.text(),gl_metadata=gl_metadata,endpoint_pixel=[endpoint[0]+p.x(),endpoint[1]+p.y()]if endpoint else None,snap_kind=vp.last_snap.kind if vp.last_snap else None,window_id=int(window.winId()),window_title=window.windowTitle(),frames=frames,eye=vec(vp.camera.eye()),forward=vec(vp.camera.forward()),mode=mode,preview=[[vec(a),vec(b)]for a,b in lines][0]if lines else None,preview_pixels=pixels[0]if pixels else None,viewport=[p.x(),p.y(),p.x()+vp.width(),p.y()+vp.height()],undo=len(vp.history.undo_stack),version=vp.scene.version,faces=len(vp.scene.mesh.faces),focused=vp.hasFocus(),pointer=pointer)
 tmp=directory/'state.tmp';tmp.write_text(json.dumps(data));tmp.replace(directory/'state.json')
timer=QTimer();timer.timeout.connect(state);timer.start(16);app.exec()
