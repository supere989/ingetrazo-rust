#!/usr/bin/env python3
"""Original OpenGL stencil capture fixture; run only on isolated X11."""
import sys,time,json,importlib.util,ctypes,subprocess,ast,inspect,textwrap,hashlib
import numpy as np
from pathlib import Path
from acceptance_context import context
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QSurfaceFormat,QVector3D,QImage
from PySide6.QtOpenGL import QOpenGLFramebufferObject,QOpenGLFramebufferObjectFormat,QOpenGLShaderProgram,QOpenGLShader,QOpenGLVertexArrayObject
from PySide6.QtWidgets import QApplication
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setDepthBufferSize(24);fmt.setStencilBufferSize(8);QSurfaceFormat.setDefaultFormat(fmt)
assert subprocess.check_output(["git","-C","ingetrazo","rev-parse","HEAD"],text=True).strip()=="6be29fe437a7cd992d4e079f71720821b25216b9"
assert Path("ingetrazo/views/viewport.py").read_bytes()==subprocess.check_output(["git","-C","ingetrazo","show","HEAD:views/viewport.py"]),"Original viewport bytes differ from frozen pin"
app=QApplication([])
from views.viewport import Viewport
from core.scene import Scene
from core.section import SectionPlane
from core.style import Style
from core.orient import orient_outward
from core.mesh import Mesh
spec=importlib.util.spec_from_file_location("reference_orient",Path("ingetrazo/tests/test_orient.py"));ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)
# New transform-feedback observation only; original shader files remain unchanged.
root=Path(sys.argv[1]);root.mkdir(exist_ok=True);name="closed-iso"
vp=Viewport();vp.resize(800,600);m=Mesh();ref._two_storey_box(m)
if name.startswith("closed") or name in ("transparent","duplicate"):m.remove_face(m.faces[1])
orient_outward(m)
vp.scene=Scene(mesh=m);vp.scene.section_planes=[SectionPlane(QVector3D(0,0,1.5),QVector3D(0,0,1),active=True)];vp.scene.show_section_planes=False;vp.scene.show_section_cuts=True;vp.style_override=Style(face_mode="shaded",sky=False,edges=False,profiles=False,section_fill_color=(1.,0.,1.));vp.camera.set_view("iso" if name.endswith("iso") else "front" if name.endswith("front") else "top");vp.camera.perspective=False;vp.camera.target=QVector3D(.5,.5,1.5);vp.camera.distance=5.;vp.show()
for _ in range(40):app.processEvents();time.sleep(.025)
vp.update()
for _ in range(10):app.processEvents();time.sleep(.025)
assert vp._faces_count==60 and vp._dback_count==60 and vp._tex_faces_count==0,(vp._faces_count,vp._dback_count,vp._tex_faces_count)
import zipfile
canonical=zipfile.ZipFile('docs/audits/section-iso-stages/source.zip').read('closed-iso.rgba')
def rgba(image):
 image=image.convertToFormat(QImage.Format_RGBA8888)
 return np.frombuffer(image.constBits(),np.uint8).reshape(image.height(),image.bytesPerLine())[:,:image.width()*4].copy().tobytes()
normal=vp.grabFramebuffer();assert rgba(normal)==canonical,"Untouched original fullframe differs"
lib=ctypes.CDLL('libGL.so.1')
def api(name,restype,args):
 f=getattr(lib,name);f.restype=restype;f.argtypes=args;return f
U=ctypes.c_uint;I=ctypes.c_int;P=ctypes.c_void_p
geti=api('glGetIntegerv',None,[U,ctypes.POINTER(I)])
def integer(token):
 v=I();geti(token,ctypes.byref(v));return v.value
error=api('glGetError',U,[])
getuniform=api('glGetUniformfv',None,[U,I,ctypes.POINTER(ctypes.c_float)])
getuniformi=api('glGetUniformiv',None,[U,I,ctypes.POINTER(I)])
location=api('glGetUniformLocation',I,[U,ctypes.c_char_p])
getattri=api('glGetVertexAttribiv',None,[U,U,ctypes.POINTER(I)])
getattrf=api('glGetVertexAttribfv',None,[U,U,ctypes.POINTER(ctypes.c_float)])
getbuffer=api('glGetBufferSubData',None,[U,ctypes.c_ssize_t,ctypes.c_ssize_t,P])
bindbuffer=api('glBindBuffer',None,[U,U]);use=api('glUseProgram',None,[U]);bindbase=api('glBindBufferBase',None,[U,U,U])
vp.makeCurrent()
program=QOpenGLShaderProgram();shaderbytes={}
for kind,path in [(QOpenGLShader.Vertex,'ingetrazo/resources/shaders/basic.vert'),(QOpenGLShader.Fragment,'ingetrazo/resources/shaders/basic.frag')]:
 text=Path(path).read_bytes();assert text==subprocess.check_output(['git','-C','ingetrazo','show','HEAD:'+path.split('ingetrazo/')[1]])
 shaderbytes[path]=hashlib.sha256(text).hexdigest();assert program.addShaderFromSourceCode(kind,text.decode()),program.log()
varyings=(ctypes.c_char_p*3)(b'gl_Position',b'gl_ClipDistance',b'v_world')
api('glTransformFeedbackVaryings',None,[U,I,ctypes.POINTER(ctypes.c_char_p),U])(program.programId(),3,varyings,0x8C8C)
assert program.link(),"Original builtin transform-feedback linking refused: "+program.log()
getvary=api('glGetTransformFeedbackVarying',None,[U,U,I,ctypes.POINTER(I),ctypes.POINTER(I),ctypes.POINTER(U),ctypes.c_char_p])
layout=[]
for i in range(3):
 length=I();size=I();kind=U();text=ctypes.create_string_buffer(128);getvary(program.programId(),i,128,ctypes.byref(length),ctypes.byref(size),ctypes.byref(kind),text);layout.append([text.value.decode(),size.value,kind.value])
assert layout==[['gl_Position',1,0x8B52],['gl_ClipDistance',1,0x1406],['v_world',1,0x8B51]],layout
records=np.empty((60,8),np.float32);records.fill(np.nan)
buffer=U();api('glGenBuffers',None,[I,ctypes.POINTER(U)])(1,ctypes.byref(buffer))
oldgl=vp._gl;captured=[]
class Proxy:
 def __getattr__(self,name):return getattr(oldgl,name)
 def glDrawArrays(self,mode,first,count):
  if not captured and mode==4 and count==60 and integer(0x85B5)==vp._faces_vao.objectId():
   original=integer(0x8B8D);array=integer(0x8894);tf=integer(0x8C8F);binding=I();getattri(0,0x889F,ctypes.byref(binding));stride=I();getattri(0,0x8624,ctypes.byref(stride));assert stride.value==24 and first==0
   upload=np.empty((60,6),np.float32);bindbuffer(0x8892,binding.value);getbuffer(0x8892,0,upload.nbytes,upload.ctypes.data)
   uniforms={}
   for name,n in [('u_mvp',16),('u_clip_plane',4)]:
    values=(ctypes.c_float*n)();loc=location(original,name.encode());assert loc>=0;getuniform(original,loc,values);uniforms[name]=np.array(values,np.float32)
   enabled=I();getuniformi(original,location(original,b'u_clip_enable'),ctypes.byref(enabled));assert enabled.value==1
   model=[];model_enabled=[]
   for attribute in range(3,7):
    values=(ctypes.c_float*4)();flag=I();getattrf(attribute,0x8626,values);getattri(attribute,0x8622,ctypes.byref(flag));model.append(np.array(values,np.float32).view(np.uint32).tolist());model_enabled.append(flag.value)
   assert model_enabled==[0,0,0,0],model_enabled
   assert model==np.eye(4,dtype=np.float32).view(np.uint32).tolist(),model
   use(program.programId())
   api('glUniformMatrix4fv',None,[I,I,ctypes.c_ubyte,ctypes.POINTER(ctypes.c_float)])(location(program.programId(),b'u_mvp'),1,0,uniforms['u_mvp'].ctypes.data_as(ctypes.POINTER(ctypes.c_float)))
   api('glUniform4fv',None,[I,I,ctypes.POINTER(ctypes.c_float)])(location(program.programId(),b'u_clip_plane'),1,uniforms['u_clip_plane'].ctypes.data_as(ctypes.POINTER(ctypes.c_float)))
   api('glUniform1i',None,[I,I])(location(program.programId(),b'u_clip_enable'),1)
   bindbuffer(0x8C8E,buffer.value);api('glBufferData',None,[U,ctypes.c_ssize_t,P,U])(0x8C8E,records.nbytes,None,0x88E9);bindbase(0x8C8E,0,buffer.value)
   api('glEnable',None,[U])(0x8C89);api('glBeginTransformFeedback',None,[U])(4);oldgl.glDrawArrays(mode,first,count);api('glEndTransformFeedback',None,[])();api('glDisable',None,[U])(0x8C89)
   api('glFinish',None,[])();getbuffer(0x8C8E,0,records.nbytes,records.ctypes.data);assert error()==0 and np.isfinite(records).all()
   captured.append({'upload_bits':upload.view(np.uint32).tolist(),'camera_bits':uniforms['u_mvp'].view(np.uint32).tolist(),'section_plane_bits':uniforms['u_clip_plane'].view(np.uint32).tolist(),'clip_enable':enabled.value,'generic_model_attribute_bits':model,'model_attribute_arrays_enabled':model_enabled,'outputs_bits':records.view(np.uint32).tolist(),'varyings':layout,'shader_sha256':shaderbytes,'separately_linked_original_glsl':True,'production_instruction_identity_claim':False})
   bindbase(0x8C8E,0,tf);bindbuffer(0x8892,array);use(original)
  return oldgl.glDrawArrays(mode,first,count)
vp._gl=Proxy()
try: observed=vp.grabFramebuffer()
finally:vp._gl=oldgl
assert len(captured)==1 and rgba(observed)==canonical,"Feedback observer altered ordinary fullframe"
vp.makeCurrent();identity={key:api('glGetString',ctypes.c_char_p,[U])(token).decode()for key,token in [('vendor',0x1F00),('renderer',0x1F01),('version',0x1F02)]};vp.doneCurrent()
(root/'report.json').write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','ordinary_and_observed_fullframe_exact':True,'gl_identity':identity,'observation':captured[0]},indent=2)+'\n')
print(json.dumps(identity));vp.close()
