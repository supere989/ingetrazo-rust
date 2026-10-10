#!/usr/bin/env python3
"""Actual Qt color MSAA resolve rounding; controlled sample colors, diagnostic only."""
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
assert program.addShaderFromSourceCode(QOpenGLShader.Fragment,"#version 330 core\nuniform float red;out vec4 color;void main(){color=vec4(red,0,0,1);}")
assert program.link(),program.log();assert program.bind();vao=QOpenGLVertexArrayObject();assert vao.create();vao.bind()
camera=SimpleNamespace(projection_matrix=lambda:QMatrix4x4(),view_matrix=lambda:QMatrix4x4(),eye=lambda:QVector3D(),forward=lambda:QVector3D(0,0,-1))
lib=ctypes.CDLL("libGL.so.1")
lib.glGetMultisamplefv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_float)]
lib.glGetFramebufferAttachmentParameteriv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_int)]
lib.glBindRenderbuffer.argtypes=[ctypes.c_uint,ctypes.c_uint]
lib.glGetRenderbufferParameteriv.argtypes=[ctypes.c_uint,ctypes.c_uint,ctypes.POINTER(ctypes.c_int)]
rows=[]
f=QOpenGLFramebufferObjectFormat();f.setAttachment(QOpenGLFramebufferObject.CombinedDepthStencil);f.setSamples(4)
scene=QOpenGLFramebufferObject(8,8,f);assert scene.isValid() and scene.bind();assert scene.format().samples()==4
gl.glViewport(0,0,8,8);gl.glDisable(0x0B71);gl.glEnable(0x809D);gl.glDisable(0x0BE2)
for values in [[33,33,33,107],[33,33,33,105],[50,50,51,51],[51,51,52,52],[100,100,101,101]] + [[base + (extra if i == slot else 0) for i in range(4)] for base in [0,33] for extra in range(16) for slot in range(4)]:
 assert scene.bind();gl.glEnable(0x8E51)
 for i,red in enumerate(values):
  gl.glSampleMaski(0,1<<i);gl.glUniform1f(program.uniformLocation("red"),float(red/255.));gl.glDrawArrays(0x0004,0,3)
 gl.glDisable(0x8E51);assert gl.glGetError()==0
 image=scene.toImage().convertToFormat(__import__('PySide6.QtGui',fromlist=['QImage']).QImage.Format_RGBA8888)
 pixels=np.frombuffer(image.constBits(),np.uint8).reshape(image.height(),image.bytesPerLine())[:,:image.width()*4].reshape(image.height(),image.width(),4).copy()
 unique=np.unique(pixels.reshape(-1,4),axis=0);assert len(unique)==1
 rows.append(dict(sample_red_bytes=values,arithmetic_average=sum(values)/4,resolved_rgba=unique[0].tolist()))
report=dict(renderer=renderer,gl_version=string(0x1F02),source_pin=pin,cases=rows,scope="Actual Qt MSAA4 FBO toImage, same API used by original screenshot capture. Controlled sample byte colors only; no Vulkan comparison or production cause established.")
(out/"resolve.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report))
vao.release();program.release();del scene;del vao;del program;ctx.doneCurrent()
