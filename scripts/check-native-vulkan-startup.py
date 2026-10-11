#!/usr/bin/env python3
"""Disposable native X11 Vulkan startup/resize evidence; never touches desktop."""
import ctypes as C, hashlib, json, os, subprocess, time
from pathlib import Path
import argparse
from owned_process_cleanup import stop_owned_helper
parser=argparse.ArgumentParser();parser.add_argument("binary");parser.add_argument("output")
parser.add_argument("--document");parser.add_argument("--require-native-world",action="store_true")
args=parser.parse_args()
out=Path(args.output).resolve();out.mkdir(parents=True,exist_ok=True)
x=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1400x1000x24"],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
display=":"+x.stdout.readline().strip();env=dict(os.environ,DISPLAY=display,XDG_CONFIG_HOME=str(out/"config"),XDG_DATA_HOME=str(out/"data"),XDG_CACHE_HOME=str(out/"cache"))
if args.require_native_world:
 for key in ("INGETRAZO_VULKAN_WORLD","INGETRAZO_CAD2D_INTEGER_BLEND","INGETRAZO_NATIVE_METAL","INGETRAZO_METAL_WORLD","INGETRAZO_METAL_CAD_CALLBACKS"):
  env.pop(key,None)
command=[args.binary]+([str(Path(args.document).resolve())] if args.document else [])
lib=C.CDLL("libX11.so.6");lib.XOpenDisplay.argtypes=[C.c_char_p];lib.XOpenDisplay.restype=C.c_void_p
lib.XDefaultRootWindow.argtypes=[C.c_void_p];lib.XDefaultRootWindow.restype=C.c_ulong
lib.XQueryTree.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ulong)),C.POINTER(C.c_uint)]
lib.XFetchName.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_char_p)]
lib.XResizeWindow.argtypes=[C.c_void_p,C.c_ulong,C.c_uint,C.c_uint];lib.XFlush.argtypes=[C.c_void_p];lib.XFree.argtypes=[C.c_void_p];lib.XCloseDisplay.argtypes=[C.c_void_p]
class ClientData(C.Union):
 _fields_=[("b",C.c_char*20),("s",C.c_short*10),("l",C.c_long*5)]
class ClientMessage(C.Structure):
 _fields_=[("type",C.c_int),("serial",C.c_ulong),("send_event",C.c_int),("display",C.c_void_p),("window",C.c_ulong),("message_type",C.c_ulong),("format",C.c_int),("data",ClientData)]
class XEvent(C.Union):
 _fields_=[("client",ClientMessage),("pad",C.c_long*24)]
lib.XInternAtom.argtypes=[C.c_void_p,C.c_char_p,C.c_int];lib.XInternAtom.restype=C.c_ulong
lib.XSendEvent.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_long,C.POINTER(XEvent)]
d=lib.XOpenDisplay(display.encode())
with (out/"startup.log").open("w") as log:
 p=subprocess.Popen(command,env=env,stdout=log,stderr=log)
 try:
  if args.require_native_world:
   deadline=time.monotonic()+30
   while time.monotonic()<deadline and p.poll() is None:
    if "Graphics backend: native Vulkan; first GPU world frame presented" in (out/"startup.log").read_text():break
    time.sleep(.1)
   assert "Graphics backend: native Vulkan; first GPU world frame presented" in (out/"startup.log").read_text(),"Native world never presented: "+(out/"startup.log").read_text()
  else: time.sleep(2)
  assert p.poll() is None,"Application exited during startup"
  subprocess.run(["import","-display",display,"-window","root",str(out/"startup.png")],check=True)
  root=lib.XDefaultRootWindow(d);rr=C.c_ulong();parent=C.c_ulong();children=C.POINTER(C.c_ulong)();n=C.c_uint()
  assert lib.XQueryTree(d,root,C.byref(rr),C.byref(parent),C.byref(children),C.byref(n)),"Cannot query native window tree"
  matches=[];titles=[]
  for i in range(n.value):
   name=C.c_char_p()
   if lib.XFetchName(d,children[i],C.byref(name)):
    if name.value and name.value.startswith(b"IngeTrazo"): matches.append(children[i]);titles.append(name.value.decode())
    lib.XFree(name)
  lib.XFree(children)
  assert len(matches)==1,f"Expected one production application window, found {matches}"
  window=matches[0]
  if args.document: assert Path(args.document).name in titles[0],f"Document was not opened: {titles[0]}"
  for width,height in [(800,600),(1100,700)]:
   lib.XResizeWindow(d,window,width,height);lib.XFlush(d);time.sleep(1)
   assert p.poll() is None,"Application exited during swapchain resize"
   subprocess.run(["import","-display",display,"-window",str(window),str(out/f"resize-{width}x{height}.png")],check=True)
  event=XEvent();event.client.type=33;event.client.send_event=1;event.client.display=d;event.client.window=window
  event.client.message_type=lib.XInternAtom(d,b"WM_PROTOCOLS",0);event.client.format=32
  event.client.data.l[0]=lib.XInternAtom(d,b"WM_DELETE_WINDOW",0)
  assert lib.XSendEvent(d,window,0,0,C.byref(event)),"Cannot send native close request"
  lib.XFlush(d)
  assert p.wait(timeout=5)==0,"Clean native close did not exit normally"
  runtime=(out/"startup.log").read_text()
  if args.require_native_world:
   assert "Native Vulkan adapter:" in runtime
   assert not any(message in runtime for message in ("GPU rendering paused:","GPU submission failed:","resources quarantined")),runtime
  evidence={"command":command,"native_world_required":args.require_native_world,"title":titles[0],"clean_exit":True,"resizes":[[800,600],[1100,700]],"binary_sha256":hashlib.sha256(Path(args.binary).read_bytes()).hexdigest(),"captures":{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in out.glob("*.png")}}
  (out/"result.json").write_text(json.dumps(evidence,indent=2)+"\n")
  print("PASS: direct Vulkan startup, document title, two native swapchain resizes and clean OS close; native world required:",args.require_native_world,"; retained captures",out)
 finally:
  if p.poll() is None: p.terminate();p.wait(timeout=5)
  lib.XCloseDisplay(d)
  stop_owned_helper(x,out/"xvfb-cleanup.json",timeout=5)
