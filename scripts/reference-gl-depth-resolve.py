#!/usr/bin/env python3
"""Actual Qt FBO depth resolve via the pinned original capture method; diagnostic only."""
import ast, ctypes, hashlib, json, os, subprocess, sys
from pathlib import Path
from types import SimpleNamespace
import numpy as np
from PySide6.QtCore import Qt
from PySide6.QtGui import QGuiApplication,QOpenGLContext,QOffscreenSurface,QSurfaceFormat,QMatrix4x4,QVector3D
from PySide6.QtOpenGL import QOpenGLFramebufferObject,QOpenGLFramebufferObjectFormat,QOpenGLFunctions_3_3_Core,QOpenGLShaderProgram,QOpenGLShader,QOpenGLVertexArrayObject
source,out=map(Path,sys.argv[1:3]);out.mkdir(parents=True,exist_ok=True)
pin="6be29fe437a7cd992d4e079f71720821b25216b9"
assert subprocess.check_output(["git","-C",str(source),"rev-parse","HEAD"],text=True).strip()==pin
app=QGuiApplication([]);fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile)
ctx=QOpenGLContext();ctx.setFormat(fmt);assert ctx.create()
surface=QOffscreenSurface();surface.setFormat(ctx.format());surface.create();assert surface.isValid() and ctx.makeCurrent(surface)
gl=QOpenGLFunctions_3_3_Core();assert gl.initializeOpenGLFunctions()
def string(which):
 value=gl.glGetString(which)
 return value.decode() if isinstance(value,bytes) else str(value)
renderer=string(0x1F01);assert "NVIDIA GeForce RTX 5090" in renderer,renderer
GL_READ_FRAMEBUFFER=0x8CA8;GL_DRAW_FRAMEBUFFER=0x8CA9;GL_DEPTH_BUFFER_BIT=0x100;GL_NEAREST=0x2600;GL_FLOAT=0x1406
path=source/"views/viewport.py";tree=ast.parse(path.read_text());method=next(n for c in tree.body if isinstance(c,ast.ClassDef) for n in c.body if isinstance(n,ast.FunctionDef) and n.name=="_capture_depth")
exec(compile(ast.fix_missing_locations(ast.Module(body=[method],type_ignores=[])),str(path),"exec"))
program=QOpenGLShaderProgram();assert program.addShaderFromSourceCode(QOpenGLShader.Vertex,"#version 330 core\nvoid main(){vec2 p[3]=vec2[3](vec2(-1,-1),vec2(3,-1),vec2(-1,3));gl_Position=vec4(p[gl_VertexID],0,1);}")
assert program.addShaderFromSourceCode(QOpenGLShader.Fragment,"#version 330 core\nuniform float depth;out vec4 color;void main(){gl_FragDepth=depth;color=vec4(1);}")
assert program.link(),program.log();assert program.bind();vao=QOpenGLVertexArrayObject();assert vao.create();vao.bind()
camera=SimpleNamespace(projection_matrix=lambda:QMatrix4x4(),view_matrix=lambda:QMatrix4x4(),eye=lambda:QVector3D(),forward=lambda:QVector3D(0,0,-1))
lib=ctypes.CDLL("libGL.so.1")
lib.glGetMultisamplefv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_float)]
lib.glGetFramebufferAttachmentParameteriv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_int)]
lib.glBindRenderbuffer.argtypes=[ctypes.c_uint,ctypes.c_uint]
lib.glGetRenderbufferParameteriv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_int)]
rows=[]
for requested in [2,4]:
 f=QOpenGLFramebufferObjectFormat();f.setAttachment(QOpenGLFramebufferObject.CombinedDepthStencil);f.setSamples(requested)
 scene=QOpenGLFramebufferObject(8,8,f);assert scene.isValid() and scene.bind();samples=scene.format().samples();assert samples in [2,4]
 positions=[]
 for i in range(samples):
  xy=(ctypes.c_float*2)();lib.glGetMultisamplefv(0x8E50,i,xy);positions.append(list(xy))
 attachment=ctypes.c_int();lib.glGetFramebufferAttachmentParameteriv(0x8D40,0x8D00,0x8CD1,ctypes.byref(attachment));assert attachment.value
 lib.glBindRenderbuffer(0x8D41,attachment.value);internal=ctypes.c_int();lib.glGetRenderbufferParameteriv(0x8D41,0x8D44,ctypes.byref(internal))
 view=SimpleNamespace(_gl=gl,_scene_fbo=scene,context=lambda:ctx,camera=camera)
 gl.glViewport(0,0,8,8);gl.glEnable(0x0B71);gl.glDepthFunc(0x0207);gl.glDepthMask(True);gl.glEnable(0x809D);gl.glDisable(0x0BE2)
 for shift in range(samples):
  assert scene.bind();gl.glDisable(0x8E51);gl.glClearDepth(1.);gl.glClear(GL_DEPTH_BUFFER_BIT);gl.glEnable(0x8E51)
  values=[(1+((i+shift)%samples))/(samples+1) for i in range(samples)]
  for i,depth in enumerate(values):
   gl.glSampleMaski(0,1<<i);gl.glUniform1f(program.uniformLocation("depth"),float(depth));gl.glDrawArrays(0x0004,0,3)
  gl.glDisable(0x8E51);assert gl.glGetError()==0
  _capture_depth(view,8,8);assert view._depth_snap is not None
  buf=view._depth_snap[0];unique=np.unique(buf);assert len(unique)==1,(requested,shift,unique)
  value=float(unique[0]);matches=[i for i,d in enumerate(values) if abs(value-d)<2e-7];assert len(matches)==1,(values,value)
  rows.append(dict(requested_samples=requested,actual_samples=samples,shift=shift,sample_positions_gl_bottom_left=positions,depth_internal_format=hex(internal.value),written_depths=values,resolved_depth=value,selected_sample=matches[0],all_64_pixels_same=True))
 del view;del scene
report=dict(source_pin=pin,source_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),renderer=renderer,gl_version=string(0x1F02),cases=rows,scope="Actual original Qt combined-depth-stencil FBO capture after controlled per-sample writes; only this device/format/sample count, not general driver policy")
(out/"resolve.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report))
vao.release();program.release();del vao;del program;ctx.doneCurrent()
