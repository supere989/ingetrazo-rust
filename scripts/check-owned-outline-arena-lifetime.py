#!/usr/bin/env python3
"""Capture only newly created owned native proof windows on authorized Xwayland."""
import ctypes as C,ctypes.util,json,os,subprocess,sys,time
from pathlib import Path
from PySide6.QtGui import QGuiApplication,QImage
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
x.XMapRaised.argtypes=[C.c_void_p,C.c_ulong]
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
class XImage(C.Structure):
 _fields_=[("width",C.c_int),("height",C.c_int),("xoffset",C.c_int),("format",C.c_int),("data",C.c_void_p),("byte_order",C.c_int),("bitmap_unit",C.c_int),("bitmap_bit_order",C.c_int),("bitmap_pad",C.c_int),("depth",C.c_int),("bytes_per_line",C.c_int),("bits_per_pixel",C.c_int),("red_mask",C.c_ulong),("green_mask",C.c_ulong),("blue_mask",C.c_ulong)]
x.XGetImage.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_ulong,C.c_int];x.XGetImage.restype=C.POINTER(XImage)
x.XDestroyImage.argtypes=[C.POINTER(XImage)]
def capture_owned(w):
 # Capture only exact newly created owned client; never root or other clients.
 p=x.XGetImage(d,w,0,0,800,600,C.c_ulong(-1).value,2)
 if not p:return None
 try:
  im=p.contents
  assert im.width==800 and im.height==600 and im.bits_per_pixel==32 and im.byte_order==0
  assert (im.red_mask,im.green_mask,im.blue_mask)==(0xff0000,0xff00,0xff)
  return QImage(C.string_at(im.data,im.bytes_per_line*im.height),800,600,im.bytes_per_line,QImage.Format.Format_RGB32).copy()
 finally:x.XDestroyImage(p)
from PySide6.QtGui import QPainter,QColor
from PySide6.QtCore import qVersion
import numpy as np
binary=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
def state():
 try:return json.loads((out/"state.json").read_text())
 except (FileNotFoundError,json.JSONDecodeError):return None
def stage(name,frames=5):
 return wait(lambda:(s if (s:=state())and s["phase"]==name and s["frames"]>=frames else None),15)
def rgba(image):
 image=image.convertToFormat(QImage.Format_RGBA8888)
 return np.frombuffer(image.constBits(),dtype=np.uint8).reshape(600,800,4).copy()
old=clients();env=dict(os.environ,INGETRAZO_ARENA_PROBE_PATH=str(out),INGETRAZO_VULKAN_DEVICE="NVIDIA",INGETRAZO_CAD2D_INTEGER_BLEND="1");logfile=out/"fixture.log"
with logfile.open("w")as stream:
 child=subprocess.Popen([str(binary)],env=env,stdout=stream,stderr=stream)
 try:
  def owned():
   assert child.poll()is None,"Owned fixture exited early"
   matches=[w for w in clients()-old if prop(w,"_NET_WM_NAME")==b"IngeTrazo outline arena lifetime fixture"];assert len(matches)<=1
   return matches[0]if matches else None
  window=wait(owned);x.XMapRaised(d,window);x.XResizeWindow(d,window,800,600);x.XFlush(d)
  baseline=stage("baseline");image=wait(lambda:capture_owned(window));assert image.save(str(out/"baseline.png"));warm=stage("baseline",baseline["frames"]+20)
  for field in ["upload_calls","bytes","transfer_submissions","upload_staging_maps","buffer_allocations"]:assert baseline["stats"]["transfers"][field]==warm["stats"]["transfers"][field],("warm",field)
  assert warm["stats"]["callbacks"]>baseline["stats"]["callbacks"]
  (out/"advance").write_text("pressure");refused=stage("refused");retained=wait(lambda:capture_owned(window));assert retained.save(str(out/"refused.png"));assert np.array_equal(rgba(image),rgba(retained)),"Old pixels changed after later candidate refusal"
  assert refused["stats"]["arena"]["preserved_failures"]==1 and refused["stats"]["arena"]["candidate_rollbacks"]==1
  (out/"advance").write_text("retry");retry=stage("retry");new=wait(lambda:capture_owned(window));assert new.save(str(out/"retry.png"));assert not np.array_equal(rgba(image),rgba(new)),"Successful retry failed to publish modified geometry/pose"
  for name,img,dx,dy in [("baseline",image,0,0),("retry",new,17,11)]:
   original=QImage(800,600,QImage.Format_RGB32);original.fill(QColor(255,255,255));painter=QPainter(original);painter.fillRect(20+dx,20+dy,180,160,QColor(255,0,0));painter.fillRect(300+dx,20+dy,180,160,QColor(0,0,255));painter.end();assert original.save(str(out/(name+"-qt.png")));assert np.array_equal(rgba(img),rgba(original)),(name,"Qt whole frame mismatch")
  (out/"advance").write_text("one");one=stage("one");one_image=wait(lambda:capture_owned(window));assert one_image.save(str(out/"one.png"))
  assert one["stats"]["arena"]["uploads"]-retry["stats"]["arena"]["uploads"]==2
  assert one["stats"]["arena"]["upload_bytes"]-retry["stats"]["arena"]["upload_bytes"]==72
  assert one["stats"]["transfers"]["buffer_allocations"]==retry["stats"]["transfers"]["buffer_allocations"]
  assert one["stats"]["transfers"]["upload_staging_maps"]==retry["stats"]["transfers"]["upload_staging_maps"]
  original=QImage(800,600,QImage.Format_RGB32);original.fill(QColor(255,255,255));painter=QPainter(original);painter.fillRect(38,31,180,160,QColor(255,0,0));painter.fillRect(317,31,180,160,QColor(0,0,255));painter.end();assert original.save(str(out/"one-qt.png"));assert np.array_equal(rgba(one_image),rgba(original)),"One dirty chunk whole frame mismatch"
  close(window);assert child.wait(timeout=6)==0;text=logfile.read_text();device=json.loads(next(l.removeprefix("Renderer device: ")for l in text.splitlines()if l.startswith("Renderer device: ")));assert device["vendor_id"]==0x10de and device["device_type"]=="DISCRETE_GPU" and "RTX 5090"in device["name"]
  assert "PASS actual outline later-chunk pressure:"in text and "Native bounded GPU cleanup complete; present_fences_enabled=true; committed_bytes=0"in text
  (out/"result.json").write_text(json.dumps({"passed":True,"device":device,"baseline":baseline,"warm":warm,"refused":refused,"retry":retry,"one":one,"scope":"Actual two-chunk native pages; real later candidate admission refusal after first transfer, old full frame+pose retained, retry exact Qt RGB32, actual warm zero transfer/map/allocation delta; not large-scene throughput or Metal"},indent=2))
 finally:
  if child.poll()is None:child.terminate();child.wait(timeout=5)
