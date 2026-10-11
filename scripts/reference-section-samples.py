#!/usr/bin/env python3
"""Original OpenGL stencil capture fixture; run only on isolated X11."""
import sys,time,json,importlib.util,ctypes,subprocess
import numpy as np
from pathlib import Path
from acceptance_context import context
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QSurfaceFormat,QVector3D,QImage
from PySide6.QtOpenGL import QOpenGLFramebufferObject,QOpenGLFramebufferObjectFormat,QOpenGLShaderProgram,QOpenGLShader,QOpenGLVertexArrayObject
from PySide6.QtWidgets import QApplication
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setDepthBufferSize(24);fmt.setStencilBufferSize(8);QSurfaceFormat.setDefaultFormat(fmt)
assert subprocess.check_output(["git","-C","ingetrazo","rev-parse","HEAD"],text=True).strip()=="6be29fe437a7cd992d4e079f71720821b25216b9"
app=QApplication([])
from views.viewport import Viewport
from core.scene import Scene
from core.section import SectionPlane
from core.style import Style
from core.orient import orient_outward
from core.mesh import Mesh
spec=importlib.util.spec_from_file_location("reference_orient",Path("ingetrazo/tests/test_orient.py"));ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)
root=Path(sys.argv[1]);root.mkdir(exist_ok=True);rows=[]
for name in ["closed"]:
 vp=Viewport();vp.resize(800,600);m=Mesh();ref._two_storey_box(m)
 if name.startswith("closed") or name in ("transparent","duplicate"):m.remove_face(m.faces[1])
 orient_outward(m)
 if "reversed" in name:
  slab=next(f for f in m.faces if f.interior);slab.loop.reverse()
 if name=="transparent":
  for f in m.faces:f.attrs["opacity"]=.5
 if name=="duplicate":
  original=list(m.faces)
  for f in original:m.add_face([QVector3D(p) for p in f.vertices])
 if name=="coplanar-backface":
  m=Mesh();m.add_face([QVector3D(0,1,1.5),QVector3D(1,1,1.5),QVector3D(1,0,1.5),QVector3D(0,0,1.5)])
 vp.scene=Scene(mesh=m);vp.scene.section_planes=[SectionPlane(QVector3D(0,0,1.5),QVector3D(0,0,1),active=True)];vp.scene.show_section_planes=False;vp.scene.show_section_cuts=True;vp.style_override=Style(face_mode="shaded",sky=False,edges=False,profiles=False,section_fill_color=(1.,0.,1.));vp.camera.set_view("iso" if name.endswith("iso") else "front" if name.endswith("front") else "top");vp.camera.perspective=False;vp.camera.target=QVector3D(.5,.5,1.5);vp.camera.distance=5.;vp.show()
 for _ in range(40):app.processEvents();time.sleep(.025)
 vp.update()
 for _ in range(10):app.processEvents();time.sleep(.025)
 image=vp.grabFramebuffer();assert not image.isNull();image.save(str(root/(name+".png")))

 def rgba(im):
  im=im.convertToFormat(QImage.Format_RGBA8888)
  return np.frombuffer(im.constBits(),np.uint8).reshape(im.height(),im.bytesPerLine())[:,:im.width()*4].reshape(im.height(),im.width(),4).copy()
 vp.makeCurrent();identity_lib=ctypes.CDLL("libGL.so.1");identity_lib.glGetString.argtypes=[ctypes.c_uint];identity_lib.glGetString.restype=ctypes.c_char_p
 identity={key:identity_lib.glGetString(token).decode()for key,token in [("vendor",0x1F00),("renderer",0x1F01),("version",0x1F02)]}
 (root/"gl-identity.json").write_text(json.dumps(identity,indent=2)+"\n");vp.doneCurrent()
 assert "Mesa" in identity["version"],"Retained section oracle requires original Mesa environment"
 expected=QImage(str(Path(__file__).resolve().parents[1]/"docs/audits/section-original-gl/closed.png"))
 vp.makeCurrent();gl=vp._gl;lib=ctypes.CDLL("libGL.so.1");U=ctypes.c_uint;I=ctypes.c_int;B=ctypes.c_ubyte
 def call(name,types,*args):
  f=getattr(lib,name);f.argtypes=types;f.restype=None;f(*args)
 def integer(name):
  v=I();call("glGetIntegerv",[U,ctypes.POINTER(I)],name,ctypes.byref(v));return v.value
 def attachment(name):
  v=I();call("glGetFramebufferAttachmentParameteriv",[U,U,U,ctypes.POINTER(I)],0x8CA8,0x8CE0,name,ctypes.byref(v));return v.value
 call("glBindFramebuffer",[U,U],0x8CA8,vp._scene_fbo.handle())
 kind=attachment(0x8CD0);owner=attachment(0x8CD1)
 assert kind==0x8D41,"Expected owned multisample renderbuffer; refuse other attachment route"
 call("glBindRenderbuffer",[U,U],0x8D41,owner)
 def rb(name):
  v=I();call("glGetRenderbufferParameteriv",[U,U,ctypes.POINTER(I)],0x8D41,name,ctypes.byref(v));return v.value
 internal=rb(0x8D44);samples=rb(0x8CAB);assert internal==0x8058 and samples==4,(internal,samples)
 call("glBindFramebuffer",[U,U],0x8D40,vp._scene_fbo.handle())
 positions=[]
 for i in range(4):
  v=(ctypes.c_float*2)();call("glGetMultisamplefv",[U,U,ctypes.POINTER(ctypes.c_float)],0x8E50,i,v);positions.append(list(v))
 (root/"attachment.json").write_text(json.dumps({"type":kind,"name":owner,"format":internal,"samples":samples,"positions_bottom_origin":positions},indent=2)+"\n")
 assert np.array_equal(rgba(image),rgba(expected)),"Fresh original resolved frame differs from frozen closed reference"
 tex=U();fbo=U();call("glGenTextures",[I,ctypes.POINTER(U)],1,ctypes.byref(tex));call("glBindTexture",[U,U],0x9100,tex.value);call("glTexImage2DMultisample",[U,I,U,I,I,B],0x9100,4,internal,800,600,1)
 call("glGenFramebuffers",[I,ctypes.POINTER(U)],1,ctypes.byref(fbo));call("glBindFramebuffer",[U,U],0x8CA9,fbo.value);call("glFramebufferTexture2D",[U,U,U,U,I],0x8CA9,0x8CE0,0x9100,tex.value,0)
 call("glBlitFramebuffer",[I,I,I,I,I,I,I,I,U,U],0,0,800,600,0,0,800,600,0x4000,0x2600);assert gl.glGetError()==0,"Multisample copy failed"
 program=QOpenGLShaderProgram();assert program.addShaderFromSourceCode(QOpenGLShader.Vertex,"#version 330 core\nvoid main(){vec2 p[3]=vec2[3](vec2(-1,-1),vec2(3,-1),vec2(-1,3));gl_Position=vec4(p[gl_VertexID],0,1);}")
 assert program.addShaderFromSourceCode(QOpenGLShader.Fragment,"#version 330 core\nuniform sampler2DMS image;uniform int sample_index;out vec4 color;void main(){ivec2 p[4]=ivec2[4](ivec2(400,371),ivec2(328,299),ivec2(471,299),ivec2(400,228));color=texelFetch(image,p[int(gl_FragCoord.x)],sample_index);}")
 assert program.link(),program.log();fmt=QOpenGLFramebufferObjectFormat();fmt.setInternalTextureFormat(0x8814);target=QOpenGLFramebufferObject(4,1,fmt);assert target.isValid();vao=QOpenGLVertexArrayObject();assert vao.create()
 for capability in [0x0B71,0x0BE2,0x0B90,0x0B44,0x3000,0x8E51,0x0BD0,0x8DB9,0x0C11]:gl.glDisable(capability)
 call("glColorMask",[B,B,B,B],1,1,1,1);gl.glViewport(0,0,4,1);values=[]
 for i in range(4):
  assert target.bind();assert program.bind();vao.bind();gl.glActiveTexture(0x84C0);call("glBindTexture",[U,U],0x9100,tex.value);gl.glUniform1i(program.uniformLocation("image"),0);gl.glUniform1i(program.uniformLocation("sample_index"),i);gl.glDrawArrays(0x0004,0,3)
  a=np.empty((4,4),np.float32);call("glReadPixels",[I,I,I,I,U,U,ctypes.c_void_p],0,0,4,1,0x1908,0x1406,a.ctypes.data);assert gl.glGetError()==0;values.append(a)
 values=np.stack(values,axis=1);points=[(400,228),(328,300),(471,300),(400,371)];resolved=rgba(image)
 rows=[{"point_top_origin":p,"resolved_rgba8":resolved[p[1],p[0]].tolist(),"sample_float":values[i].tolist(),"sample_float_bits":values[i].view(np.uint32).tolist(),"sample_rgba8":np.rint(values[i]*255).astype(np.uint8).tolist()}for i,p in enumerate(points)]
 def string(token):
  f=lib.glGetString;f.argtypes=[U];f.restype=ctypes.c_char_p;return f(token).decode()
 assert "Mesa" in string(0x1F02),"Retained section oracle requires original Mesa environment"
 report={"case":"closed","pixels":rows,"attachment":{"type":kind,"name":owner,"format":internal,"samples":samples},"sample_positions_bottom_origin":positions,"renderer":string(0x1F01),"version":string(0x1F02),"source_pin":subprocess.check_output(["git","-C","ingetrazo","rev-parse","HEAD"],text=True).strip(),"fresh_resolved_exact":True,"scope":"Actual original four pixels; same-sample-count copy from owned renderbuffer, RGBA32F observation"}
 (root/"report.json").write_text(json.dumps(report,indent=2)+"\n")
 call("glDeleteFramebuffers",[I,ctypes.POINTER(U)],1,ctypes.byref(fbo));call("glDeleteTextures",[I,ctypes.POINTER(U)],1,ctypes.byref(tex));vp.doneCurrent();vp.close()
