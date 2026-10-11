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
rows=[dict(name=name,operations=[dict(kind='ellipse',center=c,radius=r,stroke=True,width={'circle':1.5,'ellipse':3.125,'tiny':.35,'large':13.5}[name],pen=[0,0,0,255],brush=[37,93,157,255])])for name,c,r in [('circle',[80.5,80.5],[17.25,17.25]),('ellipse',[80.125,80.375],[37.125,9.875]),('tiny',[80.5,80.5],[.25,.125]),('large',[320.125,190.375],[280.75,150.125])]]
app=QApplication([])
class Widget(QOpenGLWidget):
 def __init__(self):
  super().__init__();self.row=0;self.paints=0;self.records=[];self.setFormat(fmt);self.setFixedSize(800,600);self.setWindowFlag(Qt.WindowTransparentForInput);self.setWindowFlag(Qt.WindowDoesNotAcceptFocus)
 def paintGL(self):
  self.paints+=1;p=QPainter(self);p.setRenderHint(QPainter.Antialiasing,True);p.fillRect(self.rect(),QColor(255,255,255));p.setClipRect(0,0,640,400)
  gl=ctypes.CDLL('libGL.so.1');gl.glGenQueries.argtypes=[ctypes.c_int,ctypes.POINTER(ctypes.c_uint)];gl.glGetQueryObjectuiv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_uint)]
  query=ctypes.c_uint();gl.glGenQueries(1,ctypes.byref(query));gl.glBeginQuery(0x82ee,query.value)
  for op in rows[self.row]['operations']:
   p.setPen(QPen(QColor(*op['pen']),op['width']));p.setBrush(Qt.NoBrush);p.drawEllipse(QPointF(*op['center']),*op['radius'])
  gl.glEndQuery(0x82ee);count_value=ctypes.c_uint();gl.glGetQueryObjectuiv(query.value,0x8866,ctypes.byref(count_value));gl.glDeleteQueries(1,ctypes.byref(query));assert gl.glGetError()==0
  self.engine=p.paintEngine().type().value
  gl=ctypes.CDLL('libGL.so.1')
  gl.glGetVertexAttribiv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_int)]
  gl.glGetVertexAttribPointerv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_void_p)]
  gl.glBindBuffer.argtypes=[ctypes.c_uint,ctypes.c_uint]
  gl.glGetBufferSubData.argtypes=[ctypes.c_uint,ctypes.c_ssize_t,ctypes.c_ssize_t,ctypes.c_void_p]
  binding=ctypes.c_int();gl.glGetVertexAttribiv(0,0x889f,ctypes.byref(binding))
  pointer=ctypes.c_void_p();gl.glGetVertexAttribPointerv(0,0x8645,ctypes.byref(pointer))
  count=count_value.value;assert 3<=count<=515;self.submitted_vertices=count
  self.vertex_observation={'buffer':binding.value,'offset':pointer.value or 0}
  if binding.value:
   old=ctypes.c_int();gl.glGetIntegerv(0x8f36,ctypes.byref(old));gl.glBindBuffer(0x8f36,binding.value)
   size=ctypes.c_int();gl.glGetBufferParameteriv(0x8f36,0x8764,ctypes.byref(size))
   if (pointer.value or 0)+count*8<=size.value:
    values=(ctypes.c_float*(count*2))();gl.glGetBufferSubData(0x8f36,pointer.value or 0,count*8,values)
    self.vertex_observation['positions']=[[values[i*2],values[i*2+1]]for i in range(count)]
   gl.glBindBuffer(0x8f36,old.value)
  program=ctypes.c_int();gl.glGetIntegerv(0x8b8d,ctypes.byref(program));self.vertex_observation['program']=program.value
  gl.glGetAttribLocation.argtypes=[ctypes.c_uint,ctypes.c_char_p];gl.glGetAttribLocation.restype=ctypes.c_int
  gl.glGetVertexAttribfv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_float)]
  self.vertex_observation['matrix_attributes']={}
  for name in [b'pmvMatrix1',b'pmvMatrix2',b'pmvMatrix3']:
   location=gl.glGetAttribLocation(program.value,name)
   if location>=0:
    values=(ctypes.c_float*4)();gl.glGetVertexAttribfv(location,0x8626,values)
    self.vertex_observation['matrix_attributes'][name.decode()]=list(values)
  p.end()
 def capture(self):
  im=self.grabFramebuffer();assert im.size().width()==800 and im.size().height()==600
  self.makeCurrent();gl=ctypes.CDLL('libGL.so.1');gl.glGetString.restype=ctypes.c_char_p;renderer=gl.glGetString(0x1f01).decode();samples=ctypes.c_int();gl.glGetIntegerv(0x80a9,ctypes.byref(samples));self.doneCurrent()
  assert 'NVIDIA' in renderer or '5090' in renderer;assert samples.value==0 and self.engine==13
  path=out/(rows[self.row]['name']+'.png');assert im.save(str(path));self.records.append(dict(rows[self.row],framebuffer=[im.width(),im.height()],clip=[0,0,640,400],renderer=renderer,samples=samples.value,engine=self.engine,submitted_vertices=self.submitted_vertices,vertex_observation=self.vertex_observation,paints=self.paints,sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
  self.row+=1
  if self.row==len(rows):
   (out/'original.json').write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','qt':qVersion(),'scope':'actual Qt OpenGL widget painter primitive controls; not original import/view geometry','cases':self.records},indent=2)+'\n');self.close();app.quit()
  else:self.update();QTimer.singleShot(80,self.capture)
w=Widget();w.show();QTimer.singleShot(500,w.capture);QTimer.singleShot(10000,lambda:sys.exit('bounded Qt primitive capture missed completion'));raise SystemExit(app.exec())
