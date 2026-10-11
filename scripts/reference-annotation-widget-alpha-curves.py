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
rows=[]
for alpha in [0,1,127,128,230,255]:
 ink=[37,93,157,alpha]
 for name,a,b,w in [('horizontal',[20.5,20.4999],[80.5,20.4999],3.125),('narrow',[20.25,20.5],[180.75,80.25],.35),('short',[20.125,20.125],[20.2,20.15],7.125),('cross',[80.25,20.75],[20.125,100.625],13.5)]:
  row=line(f'alpha-{alpha}-{name}',a,b,w)
  if name=='cross':row['operations'].append(dict(kind='line',a=[20.25,20.75],b=[80.125,100.625],width=13.5,pen=[157,37,93,alpha],brush=[157,37,93,alpha]))
  rows.append(row)
for name,center,radius,w,alpha in [('circle',[80.5,80.5],[17.25,17.25],1.5,255),('ellipse',[80.125,80.375],[37.125,9.875],3.125,255),('ellipse-alpha',[80.125,80.375],[37.125,9.875],3.125,128),('ellipse-tiny',[80.5,80.5],[.25,.125],.35,230)]:
 rows.append(dict(name=name,operations=[dict(kind='ellipse',center=center,radius=radius,width=w,pen=[37,93,157,alpha],brush=[37,93,157,alpha])]))
app=QApplication([])
class Widget(QOpenGLWidget):
 def __init__(self):
  super().__init__();self.row=0;self.paints=0;self.records=[];self.setFormat(fmt);self.setFixedSize(800,600);self.setWindowFlag(Qt.WindowTransparentForInput);self.setWindowFlag(Qt.WindowDoesNotAcceptFocus)
 def paintGL(self):
  self.paints+=1;p=QPainter(self);p.setRenderHint(QPainter.Antialiasing,True);p.fillRect(self.rect(),QColor(255,255,255));p.setClipRect(0,0,640,400)
  for op in rows[self.row]['operations']:
   if op['kind']=='line':p.setPen(QPen(QColor(*op['pen']),op['width']));p.drawLine(QPointF(*op['a']),QPointF(*op['b']))
   elif op['kind']=='ellipse':
    p.setPen(QPen(QColor(*op['pen']),op['width']));p.setBrush(QColor(*op['brush']));p.drawEllipse(QPointF(*op['center']),*op['radius'])
   else:p.setPen(Qt.NoPen);p.setBrush(QColor(*op['brush']));p.drawPolygon(QPolygonF([QPointF(*v)for v in op['points']]))
  self.engine=p.paintEngine().type().value;p.end()
 def capture(self):
  im=self.grabFramebuffer();assert im.size().width()==800 and im.size().height()==600
  self.makeCurrent();gl=ctypes.CDLL('libGL.so.1');gl.glGetString.restype=ctypes.c_char_p;renderer=gl.glGetString(0x1f01).decode();samples=ctypes.c_int();gl.glGetIntegerv(0x80a9,ctypes.byref(samples));self.doneCurrent()
  assert 'NVIDIA' in renderer or '5090' in renderer;assert samples.value==0 and self.engine==13
  path=out/(rows[self.row]['name']+'.png');assert im.save(str(path));self.records.append(dict(rows[self.row],framebuffer=[im.width(),im.height()],clip=[0,0,640,400],renderer=renderer,samples=samples.value,engine=self.engine,paints=self.paints,sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
  self.row+=1
  if self.row==len(rows):
   (out/'original.json').write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','qt':qVersion(),'scope':'actual Qt OpenGL widget painter primitive controls; not original import/view geometry','cases':self.records},indent=2)+'\n');self.close();app.quit()
  else:self.update();QTimer.singleShot(80,self.capture)
w=Widget();w.show();QTimer.singleShot(500,w.capture);QTimer.singleShot(10000,lambda:sys.exit('bounded Qt primitive capture missed completion'));raise SystemExit(app.exec())
