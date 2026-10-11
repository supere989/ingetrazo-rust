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
root=Path(sys.argv[1]);root.mkdir(exist_ok=True);rows=[]
for stage in [None,0,1,2,3]:
 name="closed-iso"
 output_root=root
 root=output_root if stage is None else output_root/str(stage)
 root.mkdir(exist_ok=True)
 vp=Viewport();vp.resize(800,600);m=Mesh();ref._two_storey_box(m)
 if name.startswith("closed") or name in ("transparent","duplicate"):m.remove_face(m.faces[1])
 orient_outward(m)
 vp.scene=Scene(mesh=m);vp.scene.section_planes=[SectionPlane(QVector3D(0,0,1.5),QVector3D(0,0,1),active=True)];vp.scene.show_section_planes=False;vp.scene.show_section_cuts=True;vp.style_override=Style(face_mode="shaded",sky=False,edges=False,profiles=False,section_fill_color=(1.,0.,1.));vp.camera.set_view("iso" if name.endswith("iso") else "front" if name.endswith("front") else "top");vp.camera.perspective=False;vp.camera.target=QVector3D(.5,.5,1.5);vp.camera.distance=5.;vp.show()
 for _ in range(40):app.processEvents();time.sleep(.025)
 vp.update()
 for _ in range(10):app.processEvents();time.sleep(.025)
 assert vp._faces_count==60 and vp._dback_count==60 and vp._tex_faces_count==0,(vp._faces_count,vp._dback_count,vp._tex_faces_count)
 image=vp.grabFramebuffer();assert not image.isNull()
 normal=image.copy()
 if stage is not None:
  class PrefixStop(Exception):pass
  import views.viewport as viewport_module
  def compile_hook(method,boundary):
   text=textwrap.dedent(inspect.getsource(method));tree=ast.parse(text);fn=tree.body[0];found=0
   for index,node in enumerate(fn.body):
    segment=ast.get_source_segment(text,node) or ""
    match=(boundary==0 and isinstance(node,ast.If) and "self._dback_count > 0" in segment) or (boundary==1 and isinstance(node,ast.Expr) and "self._draw_instanced_faces(mode, style)" in segment) or (boundary==2 and isinstance(node,ast.Expr) and "self._gl.glDrawArrays(GL_TRIANGLES, 0, 6)" in segment) or (boundary==3 and isinstance(node,ast.Expr) and "self._draw_section_fill(mode, style)" in segment)
    if match:
     found+=1;offset=index+(1 if boundary==3 else 0);fn.body.insert(offset,ast.Raise(exc=ast.Call(func=ast.Name(id="PrefixStop",ctx=ast.Load()),args=[],keywords=[]),cause=None));break
   assert found==1,(method.__name__,boundary)
   ast.fix_missing_locations(tree);namespace=dict(viewport_module.__dict__);namespace["PrefixStop"]=PrefixStop
   exec(compile(tree,"<probe-local-pinned-stage>","exec"),namespace)
   return namespace[method.__name__],hashlib.sha256(text.encode()).hexdigest()
  original_paint=Viewport.paintGL;original_fill=Viewport._draw_section_fill
  hooked,hook_sha=compile_hook(original_fill if stage==2 else original_paint,stage)
  if stage==2:vp._draw_section_fill=hooked.__get__(vp,Viewport)
  else:vp.paintGL=hooked.__get__(vp,Viewport)
  vp.makeCurrent()
  try:vp.paintGL()
  except PrefixStop:pass
  else:raise AssertionError("Prefix did not stop entire paint")
  # Resolve owned scene FBO directly; grabFramebuffer would repaint the frame.
  temporary=QOpenGLFramebufferObject(800,600)
  assert temporary.isValid()
  lib0=ctypes.CDLL("libGL.so.1")
  lib0.glBindFramebuffer.argtypes=[ctypes.c_uint,ctypes.c_uint]
  lib0.glBindFramebuffer(0x8CA8,vp._scene_fbo.handle());lib0.glBindFramebuffer(0x8CA9,temporary.handle())
  lib0.glBlitFramebuffer.argtypes=[ctypes.c_int]*8+[ctypes.c_uint]*2
  lib0.glGetError.restype=ctypes.c_uint
  assert lib0.glGetError()==0,"Original prefix draw GL error"
  lib0.glBlitFramebuffer(0,0,800,600,0,0,800,600,0x4000,0x2600)
  assert lib0.glGetError()==0,"Prefix direct color resolve error"
  lib0.glBindFramebuffer(0x8CA8,temporary.handle())
  lib0.glReadPixels.argtypes=[ctypes.c_int]*4+[ctypes.c_uint]*2+[ctypes.c_void_p]
  a=np.empty((600,800,4),np.uint8)
  lib0.glReadPixels(0,0,800,600,0x1908,0x1401,a.ctypes.data)
  assert lib0.glGetError()==0,"Prefix direct color readback error"
  top=np.ascontiguousarray(a[::-1]);image=QImage(top.data,800,600,3200,QImage.Format_RGBA8888).copy()
  assert not image.isNull()
  (root/"hook.json").write_text(json.dumps({"stage":stage,"method":hooked.__name__,"pinned_method_sha256":hook_sha,"whole_paint_aborted":True},indent=2)+"\n")
  vp.paintGL=original_paint.__get__(vp,Viewport);vp._draw_section_fill=original_fill.__get__(vp,Viewport)
 image.save(str(root/(name+".png")))

 def rgba(im):
  im=im.convertToFormat(QImage.Format_RGBA8888)
  return np.frombuffer(im.constBits(),np.uint8).reshape(im.height(),im.bytesPerLine())[:,:im.width()*4].reshape(im.height(),im.width(),4).copy()
 vp.makeCurrent();identity_lib=ctypes.CDLL("libGL.so.1");identity_lib.glGetString.argtypes=[ctypes.c_uint];identity_lib.glGetString.restype=ctypes.c_char_p
 identity={key:identity_lib.glGetString(token).decode()for key,token in [("vendor",0x1F00),("renderer",0x1F01),("version",0x1F02)]}
 (root/"gl-identity.json").write_text(json.dumps(identity,indent=2)+"\n");vp.doneCurrent()
 assert "Mesa" in identity["version"],"Retained section oracle requires original Mesa environment"
 expected=QImage(str(Path(__file__).resolve().parents[1]/"docs/audits/section-original-gl/closed-iso.png"))
 vp.makeCurrent();gl=vp._gl;lib=ctypes.CDLL("libGL.so.1");U=ctypes.c_uint;I=ctypes.c_int;B=ctypes.c_ubyte
 def call(name,types,*args):
  f=getattr(lib,name);f.argtypes=types;f.restype=None;f(*args)
 def integer(name):
  v=I();call("glGetIntegerv",[U,ctypes.POINTER(I)],name,ctypes.byref(v));return v.value
 def attachment(name,slot=0x8CE0):
  v=I();call("glGetFramebufferAttachmentParameteriv",[U,U,U,ctypes.POINTER(I)],0x8CA8,slot,name,ctypes.byref(v));return v.value
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
 assert np.array_equal(rgba(normal),rgba(expected)),"Unmodified original full resolved frame differs from frozen closed-iso reference"
 (root/"closed-iso.rgba").write_bytes(rgba(image).tobytes())
 depth_kind=attachment(0x8CD0,0x8D00);depth_owner=attachment(0x8CD1,0x8D00)
 assert depth_kind==0x8D41,"Expected owned original depth renderbuffer"
 call("glBindRenderbuffer",[U,U],0x8D41,depth_owner)
 depth_internal=rb(0x8D44);depth_samples=rb(0x8CAB);assert depth_samples==4
 depth_attachment={"type":depth_kind,"name":depth_owner,"format":depth_internal,"samples":depth_samples}
 camera_values=(ctypes.c_float*16)();call("glGetUniformfv",[U,I,ctypes.POINTER(ctypes.c_float)],vp._program.programId(),vp._loc_mvp,camera_values)
 camera_bits=np.array(camera_values,np.float32).view(np.uint32).tolist()
 (root/"depth-attachment.json").write_text(json.dumps(depth_attachment,indent=2)+"\n")
 tex=U();fbo=U();call("glGenTextures",[I,ctypes.POINTER(U)],1,ctypes.byref(tex));call("glBindTexture",[U,U],0x9100,tex.value);call("glTexImage2DMultisample",[U,I,U,I,I,B],0x9100,4,internal,800,600,1)
 call("glGenFramebuffers",[I,ctypes.POINTER(U)],1,ctypes.byref(fbo));call("glBindFramebuffer",[U,U],0x8CA9,fbo.value);call("glFramebufferTexture2D",[U,U,U,U,I],0x8CA9,0x8CE0,0x9100,tex.value,0)
 call("glBlitFramebuffer",[I,I,I,I,I,I,I,I,U,U],0,0,800,600,0,0,800,600,0x4000,0x2600);assert gl.glGetError()==0,"Multisample copy failed"
 # Post-frame read-only same-sample-count depth copy. No source draw hooks.
 depth_tex=U();depth_fbo=U();call("glGenTextures",[I,ctypes.POINTER(U)],1,ctypes.byref(depth_tex));call("glBindTexture",[U,U],0x9100,depth_tex.value);call("glTexImage2DMultisample",[U,I,U,I,I,B],0x9100,4,depth_internal,800,600,1)
 call("glGenFramebuffers",[I,ctypes.POINTER(U)],1,ctypes.byref(depth_fbo));call("glBindFramebuffer",[U,U],0x8D40,depth_fbo.value)
 slot=0x821A if depth_internal in (0x88F0,0x8CAD) else 0x8D00
 call("glFramebufferTexture2D",[U,U,U,U,I],0x8D40,slot,0x9100,depth_tex.value,0);call("glDrawBuffer",[U],0);call("glReadBuffer",[U],0)
 lib.glCheckFramebufferStatus.argtypes=[U];lib.glCheckFramebufferStatus.restype=U;assert lib.glCheckFramebufferStatus(0x8D40)==0x8CD5
 call("glBindFramebuffer",[U,U],0x8CA8,vp._scene_fbo.handle());call("glBindFramebuffer",[U,U],0x8CA9,depth_fbo.value)
 call("glBlitFramebuffer",[I,I,I,I,I,I,I,I,U,U],0,0,800,600,0,0,800,600,0x100,0x2600);assert gl.glGetError()==0,"Same-format multisample depth copy failed"
 program=QOpenGLShaderProgram();assert program.addShaderFromSourceCode(QOpenGLShader.Vertex,"#version 330 core\nvoid main(){vec2 p[3]=vec2[3](vec2(-1,-1),vec2(3,-1),vec2(-1,3));gl_Position=vec4(p[gl_VertexID],0,1);}")
 assert program.addShaderFromSourceCode(QOpenGLShader.Fragment,"#version 330 core\nuniform sampler2DMS image;uniform sampler2DMS depth_image;uniform int sample_index;uniform int observe_depth;out vec4 color;void main(){ivec2 p[3]=ivec2[3](ivec2(297,270),ivec2(297,271),ivec2(298,296));ivec2 q=p[int(gl_FragCoord.x)];color=observe_depth==1?vec4(texelFetch(depth_image,q,sample_index).r,0,0,1):texelFetch(image,q,sample_index);}")
 assert program.link(),program.log();fmt=QOpenGLFramebufferObjectFormat();fmt.setInternalTextureFormat(0x8814);target=QOpenGLFramebufferObject(3,1,fmt);assert target.isValid();vao=QOpenGLVertexArrayObject();assert vao.create()
 for capability in [0x0B71,0x0BE2,0x0B90,0x0B44,0x3000,0x8E51,0x0BD0,0x8DB9,0x0C11]:gl.glDisable(capability)
 call("glColorMask",[B,B,B,B],1,1,1,1);gl.glViewport(0,0,3,1);values=[];depth_values=[]
 for i in range(4):
  assert target.bind();assert program.bind();vao.bind();gl.glActiveTexture(0x84C0);call("glBindTexture",[U,U],0x9100,tex.value);gl.glActiveTexture(0x84C1);call("glBindTexture",[U,U],0x9100,depth_tex.value);gl.glUniform1i(program.uniformLocation("depth_image"),1);gl.glUniform1i(program.uniformLocation("observe_depth"),0);gl.glUniform1i(program.uniformLocation("image"),0);gl.glUniform1i(program.uniformLocation("sample_index"),i);gl.glDrawArrays(0x0004,0,3)
  a=np.empty((3,4),np.float32);call("glReadPixels",[I,I,I,I,U,U,ctypes.c_void_p],0,0,3,1,0x1908,0x1406,a.ctypes.data);assert gl.glGetError()==0;values.append(a)
  gl.glUniform1i(program.uniformLocation("observe_depth"),1);gl.glDrawArrays(0x0004,0,3);a=np.empty((3,4),np.float32);call("glReadPixels",[I,I,I,I,U,U,ctypes.c_void_p],0,0,3,1,0x1908,0x1406,a.ctypes.data);assert gl.glGetError()==0;depth_values.append(a[:,0].copy())
 values=np.stack(values,axis=1);depth_values=np.stack(depth_values,axis=1);points=[(297,329),(297,328),(298,303)];resolved=rgba(image)
 rows=[{"point_top_origin":p,"resolved_rgba8":resolved[p[1],p[0]].tolist(),"sample_float":values[i].tolist(),"sample_float_bits":values[i].view(np.uint32).tolist(),"sample_rgba8":np.rint(values[i]*255).astype(np.uint8).tolist(),"sample_depth_float":depth_values[i].tolist(),"sample_depth_bits":depth_values[i].view(np.uint32).tolist()}for i,p in enumerate(points)]
 def string(token):
  f=lib.glGetString;f.argtypes=[U];f.restype=ctypes.c_char_p;return f(token).decode()
 assert "Mesa" in string(0x1F02),"Retained section oracle requires original Mesa environment"
 report={"case":"closed-iso","pixels":rows,"actual_camera_uniform_column_major_bits":camera_bits,"depth_attachment":depth_attachment,"attachment":{"type":kind,"name":owner,"format":internal,"samples":samples},"sample_positions_bottom_origin":positions,"renderer":string(0x1F01),"version":string(0x1F02),"source_pin":subprocess.check_output(["git","-C","ingetrazo","rev-parse","HEAD"],text=True).strip(),"fresh_resolved_exact":stage is None,"unmodified_full_frame_exact":True,"terminal_prefix_stage":stage,"scope":"Unmodified full-frame guard plus source terminal-prefix depth/color; no resumed render pass"}
 (root/"report.json").write_text(json.dumps(report,indent=2)+"\n")
 call("glDeleteFramebuffers",[I,ctypes.POINTER(U)],1,ctypes.byref(fbo));call("glDeleteTextures",[I,ctypes.POINTER(U)],1,ctypes.byref(tex));call("glDeleteFramebuffers",[I,ctypes.POINTER(U)],1,ctypes.byref(depth_fbo));call("glDeleteTextures",[I,ctypes.POINTER(U)],1,ctypes.byref(depth_tex));vp.doneCurrent();vp.close()

 root=output_root
