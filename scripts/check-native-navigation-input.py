#!/usr/bin/env python3
"""Owned XTEST native Vulkan input; does not claim original rendered parity."""
import ctypes as C,ctypes.util,json,os,select,subprocess,sys,tempfile,time,hashlib
from pathlib import Path
binary=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve();out.mkdir(parents=True,exist_ok=True);(out/"state.json").unlink(missing_ok=True)
def wait(fn):
 end=time.monotonic()+20
 while time.monotonic()<end:
  try:
   v=fn()
   if v:return v
  except (OSError,ValueError):pass
  time.sleep(.03)
 raise AssertionError("Native navigation observation timeout")
with tempfile.TemporaryDirectory() as runtime:
 server=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1400x1000x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);app=None;d=None
 try:
  assert select.select([server.stdout],[],[],10)[0];display=":"+server.stdout.readline().strip()
  env=dict(os.environ,DISPLAY=display,XDG_RUNTIME_DIR=runtime,VK_DRIVER_FILES="/usr/share/vulkan/icd.d/lvp_icd.json",INGETRAZO_VULKAN_DEVICE="llvmpipe",XDG_CONFIG_HOME=runtime)
  with (out/"application.log").open("w") as log:
   app=subprocess.Popen([str(binary),str(out)],env=env,stdout=log,stderr=log)
   def state():return json.loads((out/"state.json").read_text())
   initial=wait(lambda:state() if state().get("viewport") else None)
   x=C.CDLL(ctypes.util.find_library("X11"));xt=C.CDLL(ctypes.util.find_library("Xtst"));x.XOpenDisplay.argtypes=[C.c_char_p];x.XOpenDisplay.restype=C.c_void_p;d=x.XOpenDisplay(display.encode());assert d
   x.XDefaultRootWindow.argtypes=[C.c_void_p];x.XDefaultRootWindow.restype=C.c_ulong;root=x.XDefaultRootWindow(d)
   x.XQueryTree.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ulong)),C.POINTER(C.c_uint)]
   rr=C.c_ulong();pp=C.c_ulong();children=C.POINTER(C.c_ulong)();n=C.c_uint();assert x.XQueryTree(d,root,C.byref(rr),C.byref(pp),C.byref(children),C.byref(n));assert n.value
   x.XFetchName.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_char_p)];x.XFree.argtypes=[C.c_void_p]
   names=[]
   for i in range(n.value):
    name=C.c_char_p()
    if x.XFetchName(d,children[i],C.byref(name)) and name.value:
     names.append((children[i],name.value.decode(errors="replace")));x.XFree(name)
   x.XFree(children)
   (out/"window-names.json").write_text(json.dumps(names))
   window=next(w for w,name in names if name.startswith("IngeTrazo"))
   x.XSetInputFocus.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_ulong];x.XFlush.argtypes=[C.c_void_p];x.XSetInputFocus(d,window,1,0);x.XFlush(d);wait(lambda:state()["focused"])
   x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint;xt.XTestFakeKeyEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
   def key(symbol,down):assert xt.XTestFakeKeyEvent(d,x.XKeysymToKeycode(d,symbol),int(down),0);x.XFlush(d)
   rows={"initial":state()}
   key(ord("w"),True);time.sleep(.25);rows["walking"]=state();assert rows["walking"]["eye"][1]>rows["initial"]["eye"][1]+.1
   key(ord("w"),False);time.sleep(.12);rows["released"]=state();time.sleep(.15);assert state()["eye"]==rows["released"]["eye"]
   key(0xffe3,True);key(ord("w"),True);time.sleep(.15);rows["ctrl_w"]=state();assert rows["ctrl_w"]["eye"]==rows["released"]["eye"];key(ord("w"),False);key(0xffe3,False)
   key(ord("e"),True);time.sleep(.15);key(ord("e"),False);time.sleep(.08);rows["rise"]=state();assert rows["rise"]["eye"][2]>rows["released"]["eye"][2]+.05;assert rows["rise"]["tool"]=="Select"
   key(ord("q"),True);time.sleep(.15);key(ord("q"),False);time.sleep(.08);rows["descend"]=state();assert rows["descend"]["eye"][2]<rows["rise"]["eye"][2]-.05;assert rows["descend"]["tool"]=="Select"
   key(ord("w"),True);time.sleep(.1);x.XSetInputFocus(d,root,1,0);x.XFlush(d);wait(lambda:not state()["focused"]);rows["focus_lost"]=state();time.sleep(.15);assert state()["eye"]==rows["focus_lost"]["eye"]
   key(ord("w"),False);x.XSetInputFocus(d,window,1,0);x.XFlush(d);wait(lambda:state()["focused"]);time.sleep(.12);assert state()["eye"]==rows["focus_lost"]["eye"]
   rect=state()["viewport"];cx=round((rect[0]+rect[2])/2);cy=round((rect[1]+rect[3])/2)
   x.XWarpPointer.argtypes=[C.c_void_p,C.c_ulong,C.c_ulong,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_int,C.c_int];x.XWarpPointer(d,0,window,0,0,0,0,cx,cy);x.XFlush(d);time.sleep(.1)
   xt.XTestFakeButtonEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong];xt.XTestFakeRelativeMotionEvent.argtypes=[C.c_void_p,C.c_int,C.c_int,C.c_ulong]
   rows["before_look"]=state();assert xt.XTestFakeButtonEvent(d,3,1,0);x.XFlush(d);time.sleep(.12)
   assert xt.XTestFakeRelativeMotionEvent(d,100,-40,0);x.XFlush(d);time.sleep(.15);rows["looking"]=state();assert rows["looking"]["yaw"]!=rows["before_look"]["yaw"]
   assert xt.XTestFakeButtonEvent(d,3,0,0);x.XFlush(d);time.sleep(.12);rows["look_released"]=state()
   subprocess.run(["import","-display",display,"-window",str(window),str(out/"native-navigation.png")],check=True)
   assert all(row["undo"]==0 and row["version"]==initial["version"] and row["faces"]==2 for row in rows.values())
   report={"passed":True,"binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"rows":rows,"scope":"Actual XTEST keys/buttons/raw relative mouse and focused window on owned Xvfb/llvmpipe native Vulkan; not hardware or original rendered parity"}
   (out/"report.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps({"passed":True,"observations":len(rows),"scope":report["scope"]}))
 finally:
  if app is not None:
   app.terminate()
   try:app.wait(timeout=10)
   except subprocess.TimeoutExpired:app.kill();app.wait()
  if d:x.XCloseDisplay.argtypes=[C.c_void_p];x.XCloseDisplay(d)
  server.terminate();server.wait(timeout=10)
