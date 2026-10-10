#!/usr/bin/env python3
"""Actual native pointer/key events through a rendered production viewport."""
import ctypes as C,ctypes.util,json,os,subprocess,tempfile,time,select,struct,zlib
from pathlib import Path
from acceptance_context import context
def wait(fn,seconds=20):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  try:
   v=fn()
   if v:return v
  except (OSError,ValueError):pass
  time.sleep(.05)
 raise AssertionError("Timed out awaiting rendered event/state")
def main():
 subprocess.run(["cargo","build","--locked","-p","ingetrazo-gui","--example","shift_inference_render_fixture"],check=True)
 directory=Path("docs/audits/shift-inference-render");directory.mkdir(exist_ok=True)
 for p in directory.iterdir():
  if p.name in ("state.json","capture.txt","before.png","preview.png","cancelled.png","committed.png","committed.igz"):p.unlink()
 with tempfile.TemporaryDirectory(prefix="ingetrazo-reference-x11-") as t:
  server=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1200x900x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  app=None
  try:
   assert select.select([server.stdout],[],[],10)[0];display=server.stdout.readline().strip();assert display.isdigit()
   env=os.environ.copy();env["DISPLAY"]=":"+display;env["XDG_RUNTIME_DIR"]=t;env["LIBGL_ALWAYS_SOFTWARE"]="1";env.pop("WAYLAND_DISPLAY",None)
   log=open(directory/"runtime.log","w");app=subprocess.Popen(["target/debug/examples/shift_inference_render_fixture",str(directory.resolve())],env=env,stdout=log,stderr=log)
   state=lambda:json.loads((directory/"state.json").read_text())
   initial=wait(lambda:state() if state()["faces"]==0 else None)
   x=C.CDLL(ctypes.util.find_library("X11"));x.XOpenDisplay.argtypes=[C.c_char_p];x.XOpenDisplay.restype=C.c_void_p;d=x.XOpenDisplay(env["DISPLAY"].encode());assert d
   x.XDefaultRootWindow.argtypes=[C.c_void_p];x.XDefaultRootWindow.restype=C.c_ulong;root=x.XDefaultRootWindow(d)
   x.XQueryTree.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ulong)),C.POINTER(C.c_uint)]
   children=C.POINTER(C.c_ulong)();r,p,n=C.c_ulong(),C.c_ulong(),C.c_uint();x.XQueryTree(d,root,C.byref(r),C.byref(p),C.byref(children),C.byref(n));assert n.value>0
   x.XGetGeometry.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_uint),C.POINTER(C.c_uint),C.POINTER(C.c_uint),C.POINTER(C.c_uint)]
   sized=[]
   for i in range(n.value):
    rr,xx,yy,w,h,border,depth=C.c_ulong(),C.c_int(),C.c_int(),C.c_uint(),C.c_uint(),C.c_uint(),C.c_uint()
    x.XGetGeometry(d,children[i],C.byref(rr),C.byref(xx),C.byref(yy),C.byref(w),C.byref(h),C.byref(border),C.byref(depth));sized.append((w.value*h.value,children[i]))
   window=max(sized)[1]
   x.XWarpPointer.argtypes=[C.c_void_p,C.c_ulong,C.c_ulong,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_int,C.c_int];x.XFlush.argtypes=[C.c_void_p];x.XSetInputFocus.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_ulong];x.XSetInputFocus(d,window,1,0)
   def move(point):
    x.XWarpPointer(d,0,window,0,0,0,0,round(point[0]),round(point[1]));x.XFlush(d);time.sleep(.2)
   xt=C.CDLL(ctypes.util.find_library("Xtst"));xt.XTestFakeButtonEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong];xt.XTestFakeKeyEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
   def event(kind,code,point,mask):
    if kind in (4,5):assert xt.XTestFakeButtonEvent(d,code,kind==4,0)
    else:assert xt.XTestFakeKeyEvent(d,code,kind==2,0)
    x.XFlush(d);time.sleep(.15)
   def click(point):
    move(point);event(4,1,point,4);event(5,1,point,8)
   def capture(name):
    (directory/"capture.txt").write_text(name);wait(lambda:(directory/(name+".png")).is_file())
   x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint
   def key(symbol):
    code=x.XKeysymToKeycode(d,symbol);event(2,code,None,1);event(3,code,None,2)
   capture("before");click(initial["click"]);move(initial["cue"])
   cue=wait(lambda:state() if len(state()["preview"])==1 else None)
   shift=x.XKeysymToKeycode(d,0xffe1)
   event(2,shift,None,1);move(initial["wander"])
   locked=wait(lambda:state() if abs(state()["preview"][0][1][1]-3)<.03 else None)
   assert locked["axis"] is None and locked["edges"]==0 and locked["undo"]==0
   capture("locked");event(3,shift,None,2)
   released=wait(lambda:state() if abs(state()["preview"][0][1][1]-4)<.03 else None)
   assert released["axis"] is None and released["edges"]==0 and released["undo"]==0
   capture("released");key(0xff53)
   arrow=wait(lambda:state() if state()["axis"]=="x" else None)
   event(2,shift,None,1);event(3,shift,None,2)
   preserved=state();assert preserved["axis"]=="x" and abs(preserved["preview"][0][1][1]-3)<.03
   key(0xff1b);cancelled=wait(lambda:state() if not state()["preview"] else None)
   assert cancelled["edges"]==0 and cancelled["undo"]==0 and cancelled["axis"] is None
   capture("cancelled")
   report={"provenance":context(),"passed":True,"cue":cue,"locked":locked,"released":released,"arrow_preserved":preserved,"cancelled":cancelled,"scope":"Actual production viewport XTEST Shift press/wander/release without mouse movement, arrow lock preservation and Escape atomic preview cancellation; existing software GL engineering backend, independent original controller evidence not implied."}
   Path("docs/audits/shift-inference-render-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print("Native Shift gesture PASS")
  finally:
   if app:app.terminate();app.wait(timeout=5)
   server.terminate();server.communicate(timeout=5)
if __name__=="__main__":main()
