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
binary,oracle,out=map(Path,sys.argv[1:4]);out.mkdir(parents=True,exist_ok=False)
data=json.loads(oracle.read_text());assert data['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
if '--lines-only' in sys.argv:data['cases']=data['cases'][:24]
names=[r['name']for r in data['cases']]
expected=[f'alpha-{a}-{kind}'for a in [0,1,127,128,230,255]for kind in ['horizontal','narrow','short','cross']]
if '--lines-only'not in sys.argv:expected+=['circle','ellipse','ellipse-alpha','ellipse-tiny']
if '--triangles-only' in sys.argv:expected=[f'triangle-alpha-{a}'for a in [0,1,127,128,230,255]]+['triangle-overlap','triangle-sliver']
assert names==expected
if '--case' in sys.argv:
 selected=sys.argv[sys.argv.index('--case')+1];assert selected in expected
 data['cases']=[r for r in data['cases']if r['name']==selected]
records=[]
for row in data['cases']:
 case=out/row['name'];case.mkdir();control=case/'control';control.mkdir();old=clients();child=None;w=None
 env=dict(os.environ,INGETRAZO_CAD2D_INTEGER_BLEND='1',INGETRAZO_VULKAN_DEVICE='NVIDIA',INGETRAZO_ANNOTATION_WIDGET_CONTROL='1',INGETRAZO_ANNOTATION_ALPHA_UNION_PROBE='1',INGETRAZO_ANNOTATION_ORACLE=str(oracle),INGETRAZO_ANNOTATION_CASE=row['name'],INGETRAZO_ANNOTATION_CONTROL_DIR=str(control))
 def state():
  try:return json.loads((control/'state.json').read_text())
  except FileNotFoundError:return None
 with(case/'runtime.log').open('w')as stream:
  child=subprocess.Popen([str(binary)],env=env,stdout=stream,stderr=stream)
  try:
   def owned():
    assert child.poll()is None,'Actual primitive app exited before capture'
    matches=[win for win in clients()-old if prop(win,'_NET_WM_PID')==[child.pid]]
    assert len(matches)<=1;return matches[0]if matches else None
   w=wait(owned);x.XResizeWindow(d,w,800,600);x.XFlush(d)
   first=wait(state);baseline=first['submitted_serial']
   ready=wait(lambda:(s if (s:=state())and s['completed_present_serial']>baseline else None))
   im=capture_owned(w);assert im is not None;assert im.save(str(case/'native.png'))
   if '--benchmark' in sys.argv:
    before=state();deadline=time.monotonic()+2
    while time.monotonic()<deadline:app.processEvents();time.sleep(.01)
    after=state();assert before['geometry_uploads']==after['geometry_uploads'] and before['query_uploads']==after['query_uploads']
    (case/'benchmark-states.json').write_text(json.dumps([before,after],indent=2)+'\n')
   controls=None
   if '--packed-key-control' in sys.argv:
    phases=[]
    for nonce,action in enumerate(['packed-key-install','packed-key-change','packed-key-restore'],1):
     (control/'request.json').write_text(json.dumps({'action':action,'nonce':nonce}))
     accepted=wait(lambda:(s if (s:=state())and s['nonce']==nonce else None))['submitted_serial']
     phase=wait(lambda:(s if (s:=state())and s['nonce']==nonce and s['completed_present_serial']>accepted else None))
     image=capture_owned(w);assert image is not None and image.save(str(case/(action+'.png')))
     phases.append(phase)
    assert phases[1]['geometry_uploads']==phases[0]['geometry_uploads']+1
    assert phases[2]['geometry_uploads']==phases[1]['geometry_uploads']+1
    assert phases[0]['cached_bytes']==phases[1]['cached_bytes']==phases[2]['cached_bytes']
    (case/'packed-key-states.json').write_text(json.dumps(phases,indent=2)+'\n')
   if '--stencil-controls' in sys.argv and row['name']=='ellipse-alpha':
    assert ready['extent']==[800,600]
    prior=ready['submitted_serial'];(control/'request.json').write_text(json.dumps({'action':'stencil-refuse','nonce':1}))
    acknowledged=wait(lambda:(s if (s:=state())and s['nonce']==1 else None));accepted=acknowledged['submitted_serial']
    after=wait(lambda:(s if (s:=state())and s['nonce']==1 and s['completed_present_serial']>accepted else None))
    refused=capture_owned(w);assert refused is not None and refused.save(str(case/'refusal.png'))
    assert refused==im,'Stencil admission refusal changed previous full frame'
    assert after['committed_bytes']==ready['committed_bytes'] and after['geometry_uploads']==ready['geometry_uploads']
    resized=[]
    for width,height in [(704,512),(800,600)]:
     x.XResizeWindow(d,w,width,height);x.XFlush(d)
     first_resize=wait(lambda:(s if(s:=state())and s['extent']==[width,height]else None));serial=first_resize['submitted_serial']
     settled=wait(lambda:(s if(s:=state())and s['extent']==[width,height]and s['completed_present_serial']>serial else None))
     resized.append(settled)
    restored=capture_owned(w);assert restored is not None and restored.save(str(case/'resize-restored.png'))
    assert restored==im,'Stencil resize roundtrip changed full frame'
    controls={'refusal_state':after,'resize_states':resized,'full_frames_unchanged':True}
   (case/'state.json').write_text(json.dumps(ready,indent=2)+'\n');close(w);child.wait(timeout=5)
   assert child.returncode==0,'Owned native primitive close failed'
  finally:
   if child.poll()is None:
    child.terminate()
    try:child.wait(timeout=5)
    except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)
 log=(case/'runtime.log').read_text();assert 'Native Vulkan adapter: \"NVIDIA' in log and 'committed_bytes=0' in log
 records.append(dict(controls=controls,name=row['name'],pid=child.pid,window_id=w,state=ready,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),native_sha256=hashlib.sha256((case/'native.png').read_bytes()).hexdigest()))
(out/'result.json').write_text(json.dumps({'scope':'actual strict NVIDIA own-window immutable primitive painter controls','oracle_sha256':hashlib.sha256(oracle.read_bytes()).hexdigest(),'rows':records},indent=2)+'\n')
