#!/usr/bin/env python3
"""Disposable native X11 Vulkan startup/resize evidence; never touches desktop."""
import ctypes as C, os, subprocess, time
from pathlib import Path
import argparse
parser=argparse.ArgumentParser();parser.add_argument("binary");parser.add_argument("output");args=parser.parse_args()
out=Path(args.output);out.mkdir(parents=True,exist_ok=True)
x=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1400x1000x24"],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
display=":"+x.stdout.readline().strip();env=dict(os.environ,DISPLAY=display,XDG_CONFIG_HOME=str(out/"config"))
lib=C.CDLL("libX11.so.6");lib.XOpenDisplay.argtypes=[C.c_char_p];lib.XOpenDisplay.restype=C.c_void_p
lib.XDefaultRootWindow.argtypes=[C.c_void_p];lib.XDefaultRootWindow.restype=C.c_ulong
lib.XQueryTree.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ulong)),C.POINTER(C.c_uint)]
lib.XFetchName.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_char_p)]
lib.XResizeWindow.argtypes=[C.c_void_p,C.c_ulong,C.c_uint,C.c_uint];lib.XFlush.argtypes=[C.c_void_p];lib.XFree.argtypes=[C.c_void_p];lib.XCloseDisplay.argtypes=[C.c_void_p]
d=lib.XOpenDisplay(display.encode())
with (out/"startup.log").open("w") as log:
 p=subprocess.Popen([args.binary],env=env,stdout=log,stderr=log)
 try:
  time.sleep(2)
  assert p.poll() is None,"Application exited during startup"
  subprocess.run(["import","-display",display,"-window","root",str(out/"startup.png")],check=True)
  root=lib.XDefaultRootWindow(d);rr=C.c_ulong();parent=C.c_ulong();children=C.POINTER(C.c_ulong)();n=C.c_uint()
  assert lib.XQueryTree(d,root,C.byref(rr),C.byref(parent),C.byref(children),C.byref(n)),"Cannot query native window tree"
  matches=[]
  for i in range(n.value):
   name=C.c_char_p()
   if lib.XFetchName(d,children[i],C.byref(name)):
    if name.value and name.value.startswith(b"IngeTrazo"): matches.append(children[i])
    lib.XFree(name)
  lib.XFree(children)
  assert len(matches)==1,f"Expected one production application window, found {matches}"
  window=matches[0]
  for width,height in [(800,600),(1100,700)]:
   lib.XResizeWindow(d,window,width,height);lib.XFlush(d);time.sleep(1)
   assert p.poll() is None,"Application exited during swapchain resize"
   subprocess.run(["import","-display",display,"-window",str(window),str(out/f"resize-{width}x{height}.png")],check=True)
  print("PASS: direct Vulkan production UI startup and two native swapchain resizes; retained captures",out)
 finally:
  p.terminate();p.wait(timeout=5);lib.XCloseDisplay(d);x.terminate();x.wait(timeout=5)
