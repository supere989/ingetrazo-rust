#!/usr/bin/env python3
"""Capture only newly created owned native proof windows on authorized Xwayland."""
import ctypes as C,ctypes.util,json,os,subprocess,sys,time
from pathlib import Path
from PySide6.QtGui import QGuiApplication
app=QGuiApplication([])
x=C.CDLL(ctypes.util.find_library("X11"))
x.XOpenDisplay.argtypes=[C.c_char_p];x.XOpenDisplay.restype=C.c_void_p
d=x.XOpenDisplay(os.environ["DISPLAY"].encode());assert d
x.XDefaultRootWindow.argtypes=[C.c_void_p];x.XDefaultRootWindow.restype=C.c_ulong
root=x.XDefaultRootWindow(d)
x.XInternAtom.argtypes=[C.c_void_p,C.c_char_p,C.c_int];x.XInternAtom.restype=C.c_ulong
atom=lambda name:x.XInternAtom(d,name.encode(),0)
x.XGetWindowProperty.argtypes=[C.c_void_p,C.c_ulong,C.c_ulong,C.c_long,C.c_long,C.c_int,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_int),C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ubyte))]
x.XFree.argtypes=[C.c_void_p]
def prop(w,name):
 typ=C.c_ulong();fmt=C.c_int();n=C.c_ulong();after=C.c_ulong();p=C.POINTER(C.c_ubyte)()
 assert x.XGetWindowProperty(d,w,atom(name),0,4096,0,0,C.byref(typ),C.byref(fmt),C.byref(n),C.byref(after),C.byref(p))==0
 try:return C.string_at(p,n.value) if fmt.value==8 else list(C.cast(p,C.POINTER(C.c_ulong))[:n.value])
 finally:
  if p:x.XFree(p)
def clients():return set(prop(root,"_NET_CLIENT_LIST"))
x.XResizeWindow.argtypes=[C.c_void_p,C.c_ulong,C.c_uint,C.c_uint];x.XFlush.argtypes=[C.c_void_p]
class Data(C.Union):_fields_=[("l",C.c_long*5)]
class Message(C.Structure):_fields_=[("type",C.c_int),("serial",C.c_ulong),("send_event",C.c_int),("display",C.c_void_p),("window",C.c_ulong),("message_type",C.c_ulong),("format",C.c_int),("data",Data)]
class Event(C.Union):_fields_=[("message",Message),("pad",C.c_long*24)]
x.XSendEvent.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_long,C.POINTER(Event)]
def close(w):
 e=Event();e.message=Message(33,0,1,d,w,atom("WM_PROTOCOLS"),32,Data());e.message.data.l[0]=atom("WM_DELETE_WINDOW");e.message.data.l[1]=0
 assert x.XSendEvent(d,w,0,0,C.byref(e));x.XFlush(d)
def wait(fn,timeout=8):
 end=time.monotonic()+timeout
 while time.monotonic()<end:
  value=fn()
  if value:return value
  app.processEvents();time.sleep(.05)
 raise TimeoutError("Owned proof window readiness timed out")
binary=Path(sys.argv[1]);inputs=Path(sys.argv[2]);out=Path(sys.argv[3]);out.mkdir(parents=True,exist_ok=True)
original=json.loads((inputs/"original-alpha-results.json").read_text());rows=[]
base=["red-half-white","red-half-black","opacity-bucket-order","opaque-front","opaque-back","equal-opacity-forward","equal-opacity-reversed","opacity-995","opacity-zero","section-clipped"]
rounding=["rgb-tie-even","rgb-tie-odd","opacity-round-5005","opacity-round-1255"]
names=[r["case"]for r in original["cases"]]
assert names==base or names==base+rounding or names==base+rounding+["opacity-threshold-f64"]
(out/"native-ui-results.json").write_text(json.dumps({"passed":False,"cases":[],"status":"starting"}))

for row in original["cases"]:
 (out/"pending-case.json").write_text(json.dumps({"case":row["case"],"command":[str(binary)],"scope":"Own named new client window only"}))
 name=row["case"];old=clients();env=dict(os.environ,INGETRAZO_ALPHA_INPUT=str(inputs),INGETRAZO_ALPHA_CASE=name,INGETRAZO_VULKAN_DEVICE="NVIDIA")
 log=out/(name+".log")
 with log.open("w")as stream:
  child=subprocess.Popen([str(binary)],env=env,stdout=stream,stderr=stream)
  try:
   def owned():
    assert child.poll()is None,"Native proof process exited before own window capture"
    matches=[w for w in clients()-old if prop(w,"_NET_WM_NAME")==b"IngeTrazo native transparent fixture"]
    assert len(matches)<=1,"Multiple own proof windows"
    return matches[0]if matches else None
   w=wait(owned);x.XResizeWindow(d,w,800,600);x.XFlush(d);time.sleep(.6)
   image=wait(lambda:app.primaryScreen().grabWindow(w).toImage())
   assert not image.isNull() and image.width()==800 and image.height()==600
   assert image.save(str(out/(name+".png")))
   text=log.read_text();assert "NVIDIA GeForce RTX 5090"in text and "llvmpipe"not in text
   color=image.pixelColor(400,300);pixel=[color.red(),color.green(),color.blue(),color.alpha()]
   close(w);assert child.wait(timeout=5)==0
   rows.append({"case":name,"center_rgba":pixel,"owned_window_id":w,"original":row["center_rgba"],"passed":pixel==row["center_rgba"],"backend_log":log.name})
  finally:
   if child.poll()is None:
    child.terminate()
    try:child.wait(timeout=3)
    except subprocess.TimeoutExpired:child.kill();child.wait(timeout=3)
  (out/"native-ui-results.json").write_text(json.dumps({"passed":all(r["passed"]for r in rows),"cases":rows,"scope":"Actual NVIDIA Vulkan swapchain plus production World and encoded egui adapter; capture strictly newly created own named client window, clean own WM close. No desktop/root image capture."},indent=2)+"\n")
assert len(rows)==len(original["cases"])and all(r["passed"]for r in rows),rows
(out/"pending-case.json").unlink()
