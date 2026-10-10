#!/usr/bin/env python3
"""Inspect actual original renderer MSAA samples; no altered acceptance images."""
import ctypes,json,sys,time,subprocess
from pathlib import Path
import numpy as np
root=Path(__file__).resolve().parents[1];out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
source=root/'ingetrazo';assert subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
sys.path.insert(0,str(source))
from PySide6.QtGui import QSurfaceFormat,QVector3D,QImage
from PySide6.QtWidgets import QApplication
from PySide6.QtOpenGL import QOpenGLFramebufferObject,QOpenGLShaderProgram,QOpenGLShader,QOpenGLVertexArrayObject
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setDepthBufferSize(24);fmt.setStencilBufferSize(8);fmt.setSamples(4);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from core.scene import Scene
from core.style import Style
from formats.igz import load_into
from views.viewport import Viewport
if '--flat-face-color' in sys.argv or '--surface-path' in sys.argv or '--primitive-id' in sys.argv or '--float-color-bits' in sys.argv:
 class DiagnosticViewport(Viewport):
  def _compile_program(self):
   program=QOpenGLShaderProgram(self)
   for kind,name,old,new in [(QOpenGLShader.Vertex,'basic.vert','out vec3 v_color;','flat out vec3 v_color;'),(QOpenGLShader.Fragment,'basic.frag','in vec3 v_color;','flat in vec3 v_color;')]:
    shader=(source/'resources/shaders'/name).read_text();assert shader.count(old)==1
    if '--flat-face-color' in sys.argv:shader=shader.replace(old,new)
    if '--surface-path' in sys.argv and name=='basic.frag':
     final='fragColor = vec4(mix(c.rgb, u_fade_color, u_fade), c.a);';assert shader.count(final)==1
     shader=shader.replace(final,'fragColor = vec4(u_use_texture != 0 ? 1.0 : 0.0, u_use_vcolor != 0 ? 1.0 : 0.0, gl_FrontFacing ? 1.0 : 0.0, 1.0);')
    if '--primitive-id' in sys.argv and name=='basic.frag':
     final='fragColor = vec4(mix(c.rgb, u_fade_color, u_fade), c.a);';assert shader.count(final)==1
     shader=shader.replace(final,'uint id = uint(gl_PrimitiveID); fragColor = vec4(vec3(id & 255u, (id >> 8u) & 255u, (id >> 16u) & 255u)/255.0, 1.0);')
    if '--float-color-bits' in sys.argv and name=='basic.frag':
     final='fragColor = vec4(mix(c.rgb, u_fade_color, u_fade), c.a);';assert shader.count(final)==1
     # Keep opaque alpha: RGB encode the low 24 bits, high byte separately
     # selected by flag. This avoids blending corrupting encoded bits.
     channel='(bits >> 24u) & 255u' if '--float-high-byte' in sys.argv else 'bits & 255u'
     shader=shader.replace(final,'uint bits = floatBitsToUint(mix(c.rgb, u_fade_color, u_fade).r); fragColor = vec4(vec3('+channel+', (bits >> 8u) & 255u, (bits >> 16u) & 255u)/255.0, 1.0);')
    (out/name).write_text(shader)
    assert program.addShaderFromSourceCode(kind,shader),program.log()
   assert program.link(),program.log()
   return program
 Viewport=DiagnosticViewport
original=root/'docs/audits/skp-real-model-nvidia-failure/original';protocol=json.loads((original/'protocol.json').read_text())['cases'][0];c=protocol['camera'];scene=Scene();load_into(scene,original/'gondola_v20.igz')
vp=Viewport();vp.resize(800,600);vp.scene=scene;vp.style_override=Style(face_mode='shaded',sky=False,edges='--no-edges' not in sys.argv,profiles=False,background=(1.,1.,1.));vp.camera.target=QVector3D(*c['target']);vp.camera.distance=c['distance'];vp.camera.yaw=c['yaw'];vp.camera.pitch=c['pitch'];vp.camera.fov_deg=45.;vp.camera.perspective=False;vp.show()
deadline=time.monotonic()+3
while time.monotonic()<deadline:app.processEvents();time.sleep(.01)
def pixels(image):
 im=image.convertToFormat(QImage.Format_RGBA8888);return np.frombuffer(im.constBits(),np.uint8).reshape(im.height(),im.bytesPerLine())[:,:im.width()*4].reshape(im.height(),im.width(),4).copy()
vp.makeCurrent();dither_lib=ctypes.CDLL('libGL.so.1');dither_lib.glIsEnabled.argtypes=[ctypes.c_uint];dither_lib.glIsEnabled.restype=ctypes.c_ubyte;dither_before=bool(dither_lib.glIsEnabled(0x0BD0))
if '--disable-dither' in sys.argv:vp._gl.glDisable(0x0BD0)
dither_after=bool(dither_lib.glIsEnabled(0x0BD0));vp.doneCurrent()
image=vp.grabFramebuffer();assert image.save(str(out/'fresh.png'));a=pixels(image);expected=pixels(QImage(str(original/'gondola_v20-iso-source.png')));fresh_exact=bool(np.array_equal(a,expected));fresh_different=int(np.any(a!=expected,axis=2).sum())
vertex_inputs=[]
for mesh,paint,groups,mirrored in getattr(vp,'_frame_instanced',{}).values():
 base=vp._proto_base_chunk(mesh,paint);raw=base['vcol'];vertices=np.frombuffer(raw,dtype=np.float32).reshape(-1,6)
 selected=[]
 for primitive in [4,17]:
  if (primitive+1)*3<=len(vertices):selected.append({'primitive':primitive,'position_rgb':vertices[primitive*3:(primitive+1)*3].tolist()})
 vertex_inputs.append({'prototype_uid':str(base.get('uid')),'vertex_count':len(vertices),'instances':len(groups),'mirrored':bool(mirrored),'selected_triangles':selected})
(out/'vertex-inputs.json').write_text(json.dumps(vertex_inputs,indent=2)+'\n')
vp.makeCurrent();gl=vp._gl;assert vp._scene_fbo.format().samples()==4
scene_image=vp._scene_fbo.toImage();assert scene_image.save(str(out/'scene-resolved.png'));scene_pixels=pixels(scene_image)
lib=ctypes.CDLL('libGL.so.1')
def call(name,types,*args):
 fn=getattr(lib,name);fn.argtypes=types;fn.restype=None;fn(*args)
U=ctypes.c_uint;I=ctypes.c_int;B=ctypes.c_ubyte;tex=U();fbo=U()
assert vp._faces_vbo.bind();face_bytes=vp._faces_count*24;assert face_bytes<=64*1024*1024
face_data=np.empty((vp._faces_count,6),dtype=np.float32)
call('glGetBufferSubData',[U,ctypes.c_ssize_t,ctypes.c_ssize_t,ctypes.c_void_p],0x8892,0,face_bytes,face_data.ctypes.data)
assert gl.glGetError()==0;vp._faces_vbo.release()
selected=[]
for primitive in [4,17]:
 if (primitive+1)*3<=len(face_data):selected.append({'primitive':primitive,'position_rgb':face_data[primitive*3:(primitive+1)*3].tolist()})
(out/'ordinary-vertex-inputs.json').write_text(json.dumps({'vertex_count':len(face_data),'selected_triangles':selected},indent=2)+'\n')
# gl_PrimitiveID restarts at each draw; record draw-local candidates rather
# than assuming shader ID is the global ordinary-buffer triangle index.
spans=getattr(vp,'_face_spans',None)
visible,culled=vp._visible_spans(spans,vp._frame_planes,None) if spans else ([(0,vp._faces_count)],0)
draw_candidates=[]
for first,count in visible:
 for primitive in [4,17]:
  start=first+primitive*3
  if start+3<=first+count:
   draw_candidates.append({'draw_first':first,'draw_count':count,'local_primitive':primitive,'global_primitive':start//3,'position_rgb':face_data[start:start+3].tolist()})
(out/'draw-local-inputs.json').write_text(json.dumps({'ordinary_visible_spans':visible,'culled_vertices':culled,'opaque_front_copy_run':vp._fvcol_run,'candidates':draw_candidates,'scope':'Ordinary draw-local candidates; instanced and later passes not attributed.'},indent=2)+'\n')


def raw_pixels():
 data=np.empty((600,800,4),dtype=np.uint8)
 call('glReadPixels',[I,I,I,I,U,U,ctypes.c_void_p],0,0,800,600,0x1908,0x1401,data.ctypes.data)
 assert gl.glGetError()==0
 return data[::-1].copy()
resolved_target=QOpenGLFramebufferObject(800,600);assert resolved_target.isValid()
call('glBindFramebuffer',[U,U],0x8CA8,vp._scene_fbo.handle());call('glBindFramebuffer',[U,U],0x8CA9,resolved_target.handle())
call('glBlitFramebuffer',[I,I,I,I,I,I,I,I,U,U],0,0,800,600,0,0,800,600,0x4000,0x2600)
call('glBindFramebuffer',[U,U],0x8CA8,resolved_target.handle());raw_resolved=raw_pixels();np.save(out/'resolved-raw.npy',raw_resolved)

call('glGenTextures',[I,ctypes.POINTER(U)],1,ctypes.byref(tex));call('glBindTexture',[U,U],0x9100,tex.value);call('glTexImage2DMultisample',[U,I,U,I,I,B],0x9100,4,0x8058,800,600,1)
call('glGenFramebuffers',[I,ctypes.POINTER(U)],1,ctypes.byref(fbo));call('glBindFramebuffer',[U,U],0x8CA9,fbo.value);call('glFramebufferTexture2D',[U,U,U,U,I],0x8CA9,0x8CE0,0x9100,tex.value,0)
call('glBindFramebuffer',[U,U],0x8CA8,vp._scene_fbo.handle());call('glBlitFramebuffer',[I,I,I,I,I,I,I,I,U,U],0,0,800,600,0,0,800,600,0x4000,0x2600);assert gl.glGetError()==0,'MSAA-preserving copy failed'
program=QOpenGLShaderProgram();assert program.addShaderFromSourceCode(QOpenGLShader.Vertex,'#version 330 core\nvoid main(){vec2 p[3]=vec2[3](vec2(-1,-1),vec2(3,-1),vec2(-1,3));gl_Position=vec4(p[gl_VertexID],0,1);}')
assert program.addShaderFromSourceCode(QOpenGLShader.Fragment,'#version 330 core\nuniform sampler2DMS image;uniform int sample_index;out vec4 color;void main(){color=texelFetch(image,ivec2(gl_FragCoord.xy),sample_index);}')
assert program.link(),program.log();assert program.bind();vao=QOpenGLVertexArrayObject();assert vao.create();vao.bind();target=QOpenGLFramebufferObject(800,600);assert target.isValid()
for capability in [0x0B71,0x0BE2,0x0B90,0x0B44,0x3000,0x8E51,0x0BD0,0x8DB9,0x0C11]:gl.glDisable(capability)
call('glColorMask',[B,B,B,B],1,1,1,1)
gl.glViewport(0,0,800,600);gl.glActiveTexture(0x84C0);call('glBindTexture',[U,U],0x9100,tex.value);gl.glUniform1i(program.uniformLocation('image'),0)
samples=[];raw_samples=[]
for i in range(4):
 assert target.bind();assert program.bind();vao.bind()
 gl.glActiveTexture(0x84C0);call('glBindTexture',[U,U],0x9100,tex.value)
 gl.glUniform1i(program.uniformLocation('image'),0);gl.glUniform1i(program.uniformLocation('sample_index'),i)
 gl.glDrawArrays(0x0004,0,3);assert gl.glGetError()==0
 raw=raw_pixels();raw_samples.append(raw);np.save(out/f'sample-{i}-raw.npy',raw)
 im=target.toImage();assert im.save(str(out/f'sample-{i}.png'));samples.append(pixels(im))
reconstructed=((sum(x.astype(np.uint16) for x in samples)+1)//4).astype(np.uint8);different=int(np.any(reconstructed!=scene_pixels,axis=2).sum());overlay_different=int(np.any(scene_pixels!=a,axis=2).sum())
raw_sum=sum(x.astype(np.uint16) for x in raw_samples);raw_reconstructed=((raw_sum+1)//4).astype(np.uint8)
raw_different=int(np.any(raw_reconstructed!=raw_resolved,axis=2).sum())
report={'diagnostic_float_color_bits':'--float-color-bits' in sys.argv,'diagnostic_float_high_byte':'--float-high-byte' in sys.argv,'extraction_state':'explicit shader, VAO, sampler binding each sample; dither/sRGB/scissor disabled; full color mask','diagnostic_primitive_id':'--primitive-id' in sys.argv,'diagnostic_surface_path':'--surface-path' in sys.argv,'diagnostic_edges_disabled':'--no-edges' in sys.argv,'diagnostic_flat_face_color':'--flat-face-color' in sys.argv,'integer_half_down_is_acceptance_rule':False,'dither_before':dither_before,'dither_after':dither_after,'raw_reconstruction_different_pixels':raw_different,'raw_vs_qimage_scene_different_pixels':int(np.any(raw_resolved!=scene_pixels,axis=2).sum()),'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','fresh_full_image_exact':fresh_exact,'fresh_different_pixels':fresh_different,'reconstructed_resolve_different_pixels':different,'scene_vs_widget_different_pixels':overlay_different,'edge_pixels':[{'xy':[x,340],'resolved':a[340,x].tolist(),'samples':[s[340,x].tolist() for s in samples]} for x in range(586,591)],'scope':'Diagnostic original renderer sample-color inspection. A reference mismatch remains a terminal failure even though sample evidence is retained. No Rust per-sample comparison yet.'}
(out/'review.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
vao.release();program.release();del target;del resolved_target;del vao;del program;call('glDeleteFramebuffers',[I,ctypes.POINTER(U)],1,ctypes.byref(fbo));call('glDeleteTextures',[I,ctypes.POINTER(U)],1,ctypes.byref(tex));vp.doneCurrent();vp.close()

assert fresh_exact, 'Fresh source does not match retained reference; diagnostic samples retained, acceptance refused'
# Integer half-down reconstruction is a rejected hypothesis, retained as a
# diagnostic metric. Paired actual GL/Vulkan cases define resolve acceptance.
# The exact original-image gate above remains mandatory.
