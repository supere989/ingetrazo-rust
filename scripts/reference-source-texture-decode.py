#!/usr/bin/env python3
"""Actual pinned source _get_texture + actual Qt PNG decode under GL context.
Evidence harness only; runtime has no Qt or Xvfb dependency.
"""
import os,sys,json,pathlib,subprocess
out=pathlib.Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
xvfb=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","800x600x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
try:
 display=xvfb.stdout.readline().strip();assert display;os.environ["DISPLAY"]=":"+display;os.environ["QT_QPA_PLATFORM"]="xcb"
 sys.path.insert(0,"/work/ingetrazo")
 from PySide6.QtWidgets import QApplication
 from PySide6.QtGui import QImage,QColor,QOffscreenSurface,QOpenGLContext,QSurfaceFormat
 from PySide6.QtCore import qVersion
 from views.viewport import Viewport
 from PySide6.QtGui import QOpenGLFunctions
 from PySide6.QtOpenGL import QOpenGLTexture
 from core.glinfo import read_gl_info
 app=QApplication([]);surface=QOffscreenSurface();surface.create();context=QOpenGLContext();assert context.create();assert context.makeCurrent(surface)
 functions=QOpenGLFunctions(context);functions.initializeOpenGLFunctions()
 class Owner:pass
 owner=Owner();get_texture=Viewport._get_texture.__get__(owner)
 result={"source_commit":subprocess.check_output(["git","-C","/work/ingetrazo","rev-parse","HEAD"],text=True).strip(),"qt":qVersion(),"gl_info":read_gl_info(functions),"cases":{}}
 specs=[("mixed7x5",7,5,None),("mixed65x97",65,97,None),("threshold128",3,7,128),("threshold129",3,7,129),("coverage2457",64,64,2457),("coverage2458",64,64,2458)]
 for name,w,h,mode in specs:
  image=QImage(w,h,QImage.Format.Format_RGBA8888)
  for y in range(h):
   for x in range(w):
    if name.startswith("coverage"):alpha=255 if y*w+x<mode else 0
    elif mode is not None:alpha=mode
    else:alpha=[0,128,129,255][(x+3*y)%4]
    image.setPixelColor(x,y,QColor((x*37+y*17)%256,(x*19+y*53)%256,(x*11+y*29)%256,alpha))
  path=out/(name+".png");assert image.save(str(path));decoded=QImage(str(path)).convertToFormat(QImage.Format.Format_RGBA8888);raw=bytes(decoded.constBits())[:decoded.sizeInBytes()];(out/(name+".rgba")).write_bytes(raw)
  tex=get_texture(str(path));assert tex is not None
  result["cases"][name]={"width":w,"height":h,"cutout":bool(getattr(tex,"_cutout",False)),"wrap":str(tex.wrapMode(QOpenGLTexture.DirectionS)),"min":str(tex.minificationFilter()),"mag":str(tex.magnificationFilter())}
  assert get_texture(str(path)) is tex
 result["case_count"]=len(specs);(out/"original.json").write_text(json.dumps(result,indent=2)+"\n")
 # Delete live GL textures while the owning context is current.
 owner._tex_cache.clear();del tex;context.doneCurrent()
finally:
 if "context" in globals() and "surface" in globals() and context.makeCurrent(surface):
  if "owner" in globals():
   for texture in getattr(owner,"_tex_cache",{}).values():
    if texture is not None:texture.destroy()
   owner._tex_cache.clear()
  context.doneCurrent()
 xvfb.terminate();xvfb.wait(timeout=5)
