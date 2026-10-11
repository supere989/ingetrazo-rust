#!/usr/bin/env python3
"""Read the pinned source _get_texture GPU levels on the authorized NVIDIA context."""
import ctypes,hashlib,json,sys,subprocess
from pathlib import Path
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QOffscreenSurface,QOpenGLContext,QSurfaceFormat
sys.path.insert(0,"/work/ingetrazo")
from views.viewport import Viewport
from core.glinfo import read_gl_info
from PySide6.QtGui import QOpenGLFunctions
pin=subprocess.check_output(["git","-C","/work/ingetrazo","rev-parse","HEAD"],text=True).strip();assert pin=="6be29fe437a7cd992d4e079f71720821b25216b9"
source,out=map(Path,sys.argv[1:3]);out.mkdir()
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([]);surface=QOffscreenSurface();surface.setFormat(fmt);surface.create();context=QOpenGLContext();context.setFormat(fmt);assert context.create() and context.makeCurrent(surface)
functions=QOpenGLFunctions(context);functions.initializeOpenGLFunctions();info=read_gl_info(functions);assert "NVIDIA GeForce RTX 5090" in info['renderer'],info
lib=ctypes.CDLL('libGL.so.1');get=lib.glGetTexImage;get.argtypes=[ctypes.c_uint,ctypes.c_int,ctypes.c_uint,ctypes.c_uint,ctypes.c_void_p];get.restype=None
level_info=lib.glGetTexLevelParameteriv;level_info.argtypes=[ctypes.c_uint,ctypes.c_int,ctypes.c_uint,ctypes.POINTER(ctypes.c_int)];level_info.restype=None
class Owner:pass
owner=Owner();rows=[]
try:
 for path in sorted([*source.glob('*.jpg'),*source.glob('*.png')]):
  tex=Viewport._get_texture(owner,str(path));assert tex is not None;tex.bind(0);levels=[]
  for level in range(tex.mipLevels()):
   w=ctypes.c_int();h=ctypes.c_int();level_info(0x0DE1,level,0x1000,ctypes.byref(w));level_info(0x0DE1,level,0x1001,ctypes.byref(h));assert 0<w.value<=4096 and 0<h.value<=4096
   buf=(ctypes.c_ubyte*(w.value*h.value*4))();get(0x0DE1,level,0x1908,0x1401,buf);assert functions.glGetError()==0
   data=bytes(buf);filename=path.stem+f'-level{level}.rgba';(out/filename).write_bytes(data);levels.append({'level':level,'width':w.value,'height':h.value,'file':filename,'sha256':hashlib.sha256(data).hexdigest()})
  tex.release(0);rows.append({'asset':path.name,'encoded_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'levels':levels})
finally:
 for tex in owner._tex_cache.values():tex.destroy()
 owner._tex_cache.clear();context.doneCurrent()
assert rows
(out/'original.json').write_text(json.dumps({'source_pin':pin,'gl_info':info,'cases':rows},indent=2)+'\n')
