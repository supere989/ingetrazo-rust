#!/usr/bin/env python3
"""Owned Qt OpenGL widget primitive source controls; diagnostic, not import proof."""
import ctypes,json,sys,hashlib
from pathlib import Path
from PySide6.QtWidgets import QApplication
from PySide6.QtOpenGLWidgets import QOpenGLWidget
from PySide6.QtGui import QPainter,QPen,QColor,QPolygonF,QSurfaceFormat
from PySide6.QtCore import QPointF,Qt,QTimer,qVersion
out=Path(sys.argv[-1]);out.mkdir(parents=True,exist_ok=True)
assert qVersion().startswith('6.12.')
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CompatibilityProfile);fmt.setSamples(0);fmt.setAlphaBufferSize(0);fmt.setDepthBufferSize(24);fmt.setStencilBufferSize(8)
QSurfaceFormat.setDefaultFormat(fmt)
ink=[45,55,75,255]
def line(name,a,b,w):return {'name':name,'operations':[{'kind':'line','a':a,'b':b,'width':w,'pen':ink,'brush':ink}]}
rows=[{'name': 'phase-00', 'operations': [{'kind': 'polygon', 'points': [[237.5, -6.5], [193.50390625, 132.49609375], [161.5, 77.50390625]], 'width': 0, 'pen': [0, 0, 0, 255], 'brush': [17, 89, 203, 255]}]}, {'name': 'phase-01', 'operations': [{'kind': 'polygon', 'points': [[130.5, 83.49609375], [145.5, -0.99609375], [29.00390625, 25.00390625]], 'width': 0, 'pen': [0, 0, 0, 255], 'brush': [17, 89, 203, 255]}]}, {'name': 'phase-02', 'operations': [{'kind': 'polygon', 'points': [[106.5, 36.49609375], [5.49609375, 101.5], [188.00390625, 42.00390625]], 'width': 0, 'pen': [0, 0, 0, 255], 'brush': [17, 89, 203, 255]}]}, {'name': 'phase-03', 'operations': [{'kind': 'polygon', 'points': [[131.49609375, -14.49609375], [89.00390625, 102.5], [114.49609375, 142.50390625]], 'width': 0, 'pen': [0, 0, 0, 255], 'brush': [17, 89, 203, 255]}]}, {'name': 'phase-04', 'operations': [{'kind': 'polygon', 'points': [[-9.99609375, 125.50390625], [123.50390625, 39.5], [124.49609375, 7.49609375]], 'width': 0, 'pen': [0, 0, 0, 255], 'brush': [17, 89, 203, 255]}]}, {'name': 'phase-05', 'operations': [{'kind': 'polygon', 'points': [[130.50390625, 46.50390625], [84.50390625, 167.50390625], [138.50390625, 28.49609375]], 'width': 0, 'pen': [0, 0, 0, 255], 'brush': [17, 89, 203, 255]}]}, {'name': 'phase-06', 'operations': [{'kind': 'polygon', 'points': [[36.50390625, -15.5], [107.49609375, 7.00390625], [106.5, 26.00390625]], 'width': 0, 'pen': [0, 0, 0, 255], 'brush': [17, 89, 203, 255]}]}, {'name': 'phase-07', 'operations': [{'kind': 'polygon', 'points': [[192.49609375, 113.50390625], [130.49609375, 14.5], [247.50390625, 9.49609375]], 'width': 0, 'pen': [0, 0, 0, 255], 'brush': [17, 89, 203, 255]}]}, {'name': 'phase-08', 'operations': [{'kind': 'polygon', 'points': [[8.5, 170.50390625], [4.00390625, -15.50390625], [70.50390625, 113.50390625]], 'width': 0, 'pen': [0, 0, 0, 255], 'brush': [17, 89, 203, 255]}]}, {'name': 'phase-09', 'operations': [{'kind': 'polygon', 'points': [[231.5, -9.50390625], [187.50390625, 113.49609375], [206.50390625, 29.5]], 'width': 0, 'pen': [0, 0, 0, 255], 'brush': [17, 89, 203, 255]}]}, {'name': 'phase-10', 'operations': [{'kind': 'polygon', 'points': [[103.49609375, 91.5], [-21.5, 17.49609375], [210.50390625, 145.49609375]], 'width': 0, 'pen': [0, 0, 0, 255], 'brush': [17, 89, 203, 255]}]}, {'name': 'phase-11', 'operations': [{'kind': 'polygon', 'points': [[214.49609375, 46.50390625], [230.00390625, 58.00390625], [244.50390625, -19.50390625]], 'width': 0, 'pen': [0, 0, 0, 255], 'brush': [17, 89, 203, 255]}]}]
app=QApplication([])
class Widget(QOpenGLWidget):
 def __init__(self):
  super().__init__();self.row=0;self.paints=0;self.records=[];self.setFormat(fmt);self.setFixedSize(800,600);self.setWindowFlag(Qt.WindowTransparentForInput);self.setWindowFlag(Qt.WindowDoesNotAcceptFocus)
 def paintGL(self):
  self.paints+=1;p=QPainter(self);p.setRenderHint(QPainter.Antialiasing,True);p.fillRect(self.rect(),QColor(255,255,255));p.setClipRect(0,0,640,400)
  for op in rows[self.row]['operations']:
   if op['kind']=='line':p.setPen(QPen(QColor(*op['pen']),op['width']));p.drawLine(QPointF(*op['a']),QPointF(*op['b']))
   else:p.setPen(Qt.NoPen);p.setBrush(QColor(*op['brush']));p.drawPolygon(QPolygonF([QPointF(*v)for v in op['points']]))
  self.engine=p.paintEngine().type().value;p.end()
 def capture(self):
  im=self.grabFramebuffer();assert im.size().width()==800 and im.size().height()==600
  self.makeCurrent();gl=ctypes.CDLL('libGL.so.1');gl.glGetString.restype=ctypes.c_char_p;renderer=gl.glGetString(0x1f01).decode();samples=ctypes.c_int();gl.glGetIntegerv(0x80a9,ctypes.byref(samples));subpixel=ctypes.c_int();gl.glGetIntegerv(0x0d50,ctypes.byref(subpixel));version=gl.glGetString(0x1f02).decode();self.doneCurrent()
  assert 'NVIDIA' in renderer or '5090' in renderer;assert samples.value==0 and self.engine==13
  path=out/(rows[self.row]['name']+'.png');assert im.save(str(path));self.records.append(dict(rows[self.row],framebuffer=[im.width(),im.height()],clip=[0,0,640,400],renderer=renderer,samples=samples.value,subpixel_bits=subpixel.value,gl_version=version,engine=self.engine,paints=self.paints,sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
  self.row+=1
  if self.row==len(rows):
   (out/'original.json').write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','qt':qVersion(),'scope':'actual Qt OpenGL widget painter primitive controls; not original import/view geometry','cases':self.records},indent=2)+'\n');self.close();app.quit()
  else:self.update();QTimer.singleShot(80,self.capture)
w=Widget();w.show();QTimer.singleShot(500,w.capture);QTimer.singleShot(10000,lambda:sys.exit('bounded Qt primitive capture missed completion'));raise SystemExit(app.exec())
