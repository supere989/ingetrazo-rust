#!/usr/bin/env python3
"""Bounded Qt context discovery only; no source viewport parity claim."""
import json,ctypes
from PySide6.QtGui import QGuiApplication,QSurfaceFormat,QOffscreenSurface,QOpenGLContext
app=QGuiApplication([])
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile)
surface=QOffscreenSurface();surface.setFormat(fmt);surface.create()
context=QOpenGLContext();context.setFormat(fmt)
created=context.create();current=created and context.makeCurrent(surface)
result={"platform":app.platformName(),"created":created,"current":current}
if current:
 egl=ctypes.CDLL("libEGL.so.1");egl.eglGetProcAddress.restype=ctypes.c_void_p
 gl=ctypes.CFUNCTYPE(ctypes.c_char_p,ctypes.c_uint)(egl.eglGetProcAddress(b"glGetString"))
 result.update(vendor=gl(0x1f00).decode(),renderer=gl(0x1f01).decode(),version=gl(0x1f02).decode())
 context.doneCurrent()
print(json.dumps(result),flush=True)
raise SystemExit(0 if current and "NVIDIA" in result.get("vendor","") else 1)
