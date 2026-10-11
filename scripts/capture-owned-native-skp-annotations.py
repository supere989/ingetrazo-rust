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
def capture_owned(w,width=800,height=600):
 # Capture only exact newly created owned client; never root or other clients.
 p=x.XGetImage(d,w,0,0,width,height,C.c_ulong(-1).value,2)
 if not p:return None
 try:
  im=p.contents
  assert im.width==width and im.height==height and im.bits_per_pixel==32 and im.byte_order==0
  assert (im.red_mask,im.green_mask,im.blue_mask)==(0xff0000,0xff00,0xff)
  return QImage(C.string_at(im.data,im.bytes_per_line*im.height),width,height,im.bytes_per_line,QImage.Format.Format_RGB32).copy()
 finally:x.XDestroyImage(p)

import hashlib
binary,camera,scene,out=map(Path,sys.argv[1:5]);out.mkdir(parents=True,exist_ok=False)
control=out/'control';control.mkdir();old=clients();records=[]
env=dict(os.environ,INGETRAZO_CAD2D_INTEGER_BLEND='1',INGETRAZO_ANNOTATION_APP_CONTROL='1',INGETRAZO_ANNOTATION_DEPTH_SAMPLE='0',INGETRAZO_ANNOTATION_CONTROL_DIR=str(control),INGETRAZO_ANNOTATION_APP_SCENE=str(scene),INGETRAZO_ANNOTATION_APP_CAMERA=str(camera),INGETRAZO_ANNOTATION_VIEWPORT_ONLY='1')
def state():
 try:return json.loads((control/'state.json').read_text())
 except FileNotFoundError:return None
with(out/'runtime.log').open('w')as stream:
 child=subprocess.Popen([str(binary)],env=env,stdout=stream,stderr=stream)
 try:
  def owned():
   assert child.poll()is None,'Actual app exited before capture'
   hits=[w for w in clients()-old if prop(w,'_NET_WM_PID')==[child.pid]]
   assert len(hits)<=1;return hits[0]if hits else None
  w=wait(owned);x.XMapRaised(d,w);x.XFlush(d)
  first=wait(lambda:state()if state()and state()['viewport']else None)
  wait(lambda:state()['completed_present_serial']>first['submitted_serial'])
  for attempt in range(3):
   row=state();r=row['viewport'];vw=r[2]-r[0];vh=r[3]-r[1]
   if [vw,vh]==[800.,600.]:break
   width,height=row['screen_size'];before=row['submitted_serial'];nw=int(width+800-vw);nh=int(height+600-vh)
   x.XResizeWindow(d,w,nw,nh);x.XFlush(d)
   resized=wait(lambda:state()if state()['screen_size']==[float(nw),float(nh)]else None)
   wait(lambda:state()['completed_present_serial']>resized['submitted_serial'])
  row=state();rect=row['viewport'];assert rect[2]-rect[0]==800 and rect[3]-rect[1]==600
  assert all(v==int(v)for v in rect),'Fractional ROI must not be rounded away'
  assert row['annotation_prepare_error']is None,'Preparation refusal cannot masquerade as readiness'
  initial=[row['dimensions'],row['leaders']]
  assert row['annotation_ready']==(sum(initial)>0),'Readiness differs from actual imported annotation inventory'
  def take(name):
   row=state();assert row['viewport']==rect and row['annotation_prepare_error']is None
   width,height=map(int,row['screen_size']);image=wait(lambda:capture_owned(w,width,height));assert image.save(str(out/(name+'-window.png')))
   viewport=image.copy(*map(int,[rect[0],rect[1],800,600]));assert viewport.save(str(out/(name+'.png')))
   records.append(dict(name=name,state=row,sha256=hashlib.sha256((out/(name+'.png')).read_bytes()).hexdigest()))
  take('annotations-on')
  for nonce,action,name,counts in [(1,'annotations-off','annotations-off',[0,0]),(2,'annotations-on','annotations-restored',initial)]:
   before=state()['submitted_serial'];temp=control/'request.tmp';temp.write_text(json.dumps(dict(nonce=nonce,action=action)));temp.replace(control/'request.json')
   first=wait(lambda:state()if state()['nonce']==nonce else None)
   wait(lambda:state()['nonce']==nonce and state()['completed_present_serial']>first['submitted_serial'] and [state()['dimensions'],state()['leaders']]==counts)
   assert state()['annotation_ready']==(sum(counts)>0);take(name)
  close(w);assert child.wait(timeout=5)==0
  text=(out/'runtime.log').read_text();assert 'NVIDIA GeForce RTX 5090'in text and 'llvmpipe'not in text
  assert 'Native bounded GPU cleanup complete; present_fences_enabled=true; committed_bytes=0'in text,text[-2000:]
  result=dict(passed=True,records=records,owned_window_only=True,viewport=rect,depth_sample_policy='explicit experimental index0; paired MSAA edge mapping still required',binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),scene_sha256=hashlib.sha256(scene.read_bytes()).hexdigest())
  (out/'capture.json').write_text(json.dumps(result,indent=2)+'\n')
 finally:
  if child.poll()is None:
   child.terminate()
   try:child.wait(timeout=5)
   except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)
