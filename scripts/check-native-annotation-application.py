#!/usr/bin/env python3
"""Disposable native X11 Vulkan startup/resize evidence; never touches desktop."""
import ctypes as C, os, subprocess, time, json, hashlib, select
from pathlib import Path
import argparse
from owned_process_cleanup import stop_owned_helper
parser=argparse.ArgumentParser();parser.add_argument("binary");parser.add_argument("output");parser.add_argument("--source",required=True);args=parser.parse_args();assert len(args.source)==40 and all(c in "0123456789abcdef" for c in args.source)
out=Path(args.output);out.mkdir(parents=True,exist_ok=True)
x=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1400x1000x24"],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
assert select.select([x.stdout],[],[],5)[0],"Owned Xvfb startup timeout"
display=":"+x.stdout.readline().strip();control=out/"control";control.mkdir(exist_ok=True)
env=dict(os.environ,DISPLAY=display,XDG_CONFIG_HOME=str(out/"config"),INGETRAZO_CAD2D_INTEGER_BLEND="1",INGETRAZO_ANNOTATION_APP_CONTROL="1",INGETRAZO_ANNOTATION_DEPTH_SAMPLE="0",INGETRAZO_ANNOTATION_ORACLE=str(Path(__file__).resolve().parents[1]/"docs/audits/native-annotation-source/original.json"),INGETRAZO_ANNOTATION_CONTROL_DIR=str(control))
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

from PySide6.QtGui import QImage,QColor,QPainter
from PySide6.QtCore import qVersion
import numpy as np
assert qVersion().startswith("6.12."),"Pinned Qt6.12 reference required"
def wait(fn):
 deadline=time.monotonic()+8
 while time.monotonic()<deadline:
  result=fn()
  if result:return result
  assert p.poll() is None,"Owned native app exited before phase readiness"
  time.sleep(.02)
 raise TimeoutError("Annotation phase readiness timeout")
def state():
 try:return json.loads((control/"state.json").read_text())
 except FileNotFoundError:return None
def read_image(path):
 image=QImage(str(path));assert not image.isNull() and image.width()==1000 and image.height()==700,"Missing/unreadable/wrong-extent PNG"
 image=image.convertToFormat(QImage.Format_RGBA8888)
 return np.frombuffer(image.constBits(),np.uint8).reshape(700,image.bytesPerLine())[:,:1000*4].reshape(700,1000,4).copy()
def reference(alternate=False,clear=False,second=True):
 image=QImage(640,400,QImage.Format_RGBA8888);image.fill(QColor("white"))
 if not clear:
  def mul(x,a):
   t=x*a+128;return (t+(t>>8))>>8
  def pixel(value):
   r,g,b,a=[mul(v,128)for v in value];return (a<<24)|(r<<16)|(g<<8)|b
  first=QImage(20,1,QImage.Format_ARGB32_Premultiplied)
  for x in range(20):first.setPixel(x,0,pixel([60,10,20,80]if x<10 else[10,50,30,90]))
  later=QImage(20,1,QImage.Format_ARGB32_Premultiplied);later.fill(pixel([0,0,60,60]if alternate else[0,60,0,60]))
  painter=QPainter(image);painter.drawImage(20,20,first)
  if second:painter.drawImage(25,20,later)
  painter.end()
 return image
def pixels(image):
 image=image.convertToFormat(QImage.Format_RGBA8888)
 return np.frombuffer(image.constBits(),np.uint8).reshape(400,image.bytesPerLine())[:,:640*4].reshape(400,640,4).copy()
def capture(name,expected):
 path=out/(name+".png");subprocess.run(["import","-display",display,"-window",str(window),str(path)],check=True,timeout=5)
 actual=read_image(path);different=np.any(actual!=expected,axis=2)
 return dict(name=name,image_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),different_pixels=int(different.sum()),state=state())
rows=[];p=None
try:
 with(out/"runtime.log").open("w")as log:
  p=subprocess.Popen([args.binary],env=env,stdout=log,stderr=log)
  wait(lambda:state()and state()["viewport"]and state()["submitted_serial"]>1)
  root=lib.XDefaultRootWindow(d);rr=C.c_ulong();parent=C.c_ulong();children=C.POINTER(C.c_ulong)();n=C.c_uint();assert lib.XQueryTree(d,root,C.byref(rr),C.byref(parent),C.byref(children),C.byref(n))
  matches=[]
  for i in range(n.value):
   name=C.c_char_p()
   if lib.XFetchName(d,children[i],C.byref(name)):
    if name.value==b"IngeTrazo actual application annotation probe":matches.append(children[i])
    lib.XFree(name)
  lib.XFree(children);assert len(matches)==1;window=matches[0];lib.XResizeWindow(d,window,1000,700);lib.XFlush(d);time.sleep(.3)
  def take(name):
   path=out/(name+".png");subprocess.run(["import","-display",display,"-window",str(window),str(path)],check=True,timeout=5);return read_image(path)
  def roi(image):
   x0,y0,x1,y1=[int(v)for v in state()["viewport"]];return image[y0:y1,x0:x1]
  base=take("baseline");base_roi=roi(base);assert base_roi.size>0 and np.any(base_roi[:,:,:3]!=0),"Black startup cannot prove empty viewport"
  def action(nonce,name,held=False):
   previous=state();request=control/"request.tmp";request.write_text(json.dumps(dict(nonce=nonce,action=name)));request.rename(control/"request.json")
   value=wait(lambda:(v if(v:=state())and v["nonce"]==nonce and(held or v["submitted_serial"]>previous["submitted_serial"])else None))
   if held:
    time.sleep(.1);assert state()["submitted_serial"]==value["submitted_serial"]
   image=take(str(nonce)+"-"+name);rows.append(dict(name=name,state=state(),image_sha256=hashlib.sha256((out/(str(nonce)+"-"+name+".png")).read_bytes()).hexdigest()));return image
  add=action(1,"add");assert state()["dimensions"]==1 and np.any(roi(add)!=base_roi),"First addition produced no actual viewport pixels"
  delete=action(2,"delete");assert state()["dimensions"]==0 and state()["cached_bytes"]==0 and np.array_equal(roi(delete),base_roi),"Last deletion did not restore exact empty viewport"
  add2=action(3,"add");assert np.array_equal(roi(add2),roi(add)),"Same annotation restored different viewport"
  refused=action(4,"refuse",True);assert state()["pending_textures"]>0 and np.array_equal(refused,add2),"Refused app frame changed presentation or lost new texture delta"
  retry=action(5,"retry");assert not state()["held_refusal"] and state()["pending_textures"]==0 and np.any(roi(retry)!=roi(add)),"Retry did not publish replacement"
  x,y,_,_=state()["viewport"];sample=retry[int(y)+25,int(x)+25];assert sample.tolist()==[255,0,0,255],f"Deferred actual managed image lost on retry: {sample}"
  delete2=action(6,"delete");assert state()["dimensions"]==0 and state()["cached_bytes"]==0
  event=XEvent();event.client.type=33;event.client.send_event=1;event.client.display=d;event.client.window=window;event.client.message_type=lib.XInternAtom(d,b"WM_PROTOCOLS",0);event.client.format=32;event.client.data.l[0]=lib.XInternAtom(d,b"WM_DELETE_WINDOW",0);assert lib.XSendEvent(d,window,0,0,C.byref(event));lib.XFlush(d);assert p.wait(timeout=5)==0
 log=(out/"runtime.log").read_text();assert "committed_bytes=0"in log
 report=dict(source_sha=args.source,binary_sha256=hashlib.sha256(Path(args.binary).read_bytes()).hexdigest(),rows=rows,passed=True,scope="Actual IngeTrazoApp update_ui/render_viewport mid-UI add/delete/refusal/retry with new managed texture; source pixel/font/MSAA parity remains open")
 (out/"result.json").write_text(json.dumps(report,indent=2)+"\n");print("PASS actual app add/delete/held frame/managed texture retry")
except BaseException as error:
 (out/"failure.json").write_text(json.dumps(dict(source_sha=args.source,error=repr(error),rows=rows,last_state=state()),indent=2)+"\n");raise
finally:
 if p is not None:stop_owned_helper(p,out/"app-cleanup.json",timeout=5)
 lib.XCloseDisplay(d);stop_owned_helper(x,out/"xvfb-cleanup.json",timeout=5)
