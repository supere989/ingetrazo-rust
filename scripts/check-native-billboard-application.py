#!/usr/bin/env python3
"""Disposable native X11 Vulkan startup/resize evidence; never touches desktop."""
import ctypes as C, os, subprocess, time, json, hashlib, select
from pathlib import Path
import argparse
from owned_process_cleanup import stop_owned_helper
parser=argparse.ArgumentParser();parser.add_argument("binary");parser.add_argument("output");parser.add_argument("--input",required=True);parser.add_argument("--source",required=True);args=parser.parse_args();assert len(args.source)==40 and all(c in "0123456789abcdef" for c in args.source)
out=Path(args.output);out.mkdir(parents=True,exist_ok=True)
xvfb_log=(out/"xvfb.log").open("w")
xvfb=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1400x1000x24"],stdout=subprocess.PIPE,stderr=xvfb_log,text=True)
assert select.select([xvfb.stdout],[],[],5)[0],"Owned Xvfb startup timeout"
display=":"+xvfb.stdout.readline().strip();control=out/"control";control.mkdir(exist_ok=True)
env=dict(os.environ,DISPLAY=display,XDG_CONFIG_HOME=str(out/"config"),INGETRAZO_BILLBOARD_CONTROL_DIR=str(control))
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
d=None;deadline=time.monotonic()+5
while time.monotonic()<deadline and d is None:
 if xvfb.poll()is not None:break
 d=lib.XOpenDisplay(display.encode())
 if not d:time.sleep(.02)
if not d:
 stop_owned_helper(xvfb,out/"xvfb-cleanup.json",timeout=5);xvfb_log.close();raise RuntimeError("Owned Xvfb connection readiness failed")


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
 image=QImage(str(path));expected=state()["screen_size"];assert not image.isNull() and [image.width(),image.height()]==expected,"Missing/unreadable/wrong-extent owned PNG"
 image=image.convertToFormat(QImage.Format_RGBA8888)
 return np.frombuffer(image.constBits(),np.uint8).reshape(image.height(),image.bytesPerLine())[:,:image.width()*4].reshape(image.height(),image.width(),4).copy()
rows=[];p=None
try:
 with(out/"runtime.log").open("w")as log:
  p=subprocess.Popen([args.binary,args.input],env=env,stdout=log,stderr=log)
  wait(lambda:state()and state()["viewport"]and state()["observation"]["completed_present_serial"]>1)
  root=lib.XDefaultRootWindow(d);rr=C.c_ulong();parent=C.c_ulong();children=C.POINTER(C.c_ulong)();n=C.c_uint();assert lib.XQueryTree(d,root,C.byref(rr),C.byref(parent),C.byref(children),C.byref(n))
  matches=[];observed=[]
  lib.XGetWindowProperty.argtypes=[C.c_void_p,C.c_ulong,C.c_ulong,C.c_long,C.c_long,C.c_int,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_int),C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ubyte))]
  pid_atom=lib.XInternAtom(d,b"_NET_WM_PID",0)
  for i in range(n.value):
   name=C.c_char_p()
   if lib.XFetchName(d,children[i],C.byref(name)):
    observed.append(name.value.decode(errors="replace"))
    actual_type=C.c_ulong();fmt=C.c_int();items=C.c_ulong();after=C.c_ulong();data=C.POINTER(C.c_ubyte)()
    if lib.XGetWindowProperty(d,children[i],pid_atom,0,1,0,6,C.byref(actual_type),C.byref(fmt),C.byref(items),C.byref(after),C.byref(data))==0 and data:
     if fmt.value==32 and items.value==1 and C.cast(data,C.POINTER(C.c_ulong))[0]==p.pid:matches.append(children[i])
     lib.XFree(data)
    lib.XFree(name)
  lib.XFree(children);assert len(matches)==1,f"Owned process window identity absent/ambiguous: {observed}";window=matches[0];before_resize=state()["observation"]["submitted_serial"];lib.XResizeWindow(d,window,1000,700);lib.XFlush(d);wait(lambda:state()["screen_size"]==[1000.0,700.0] and state()["observation"]["completed_present_serial"]>before_resize+1)
  def take(name):
   path=out/(name+".png");subprocess.run(["import","-display",display,"-window",str(window),str(path)],check=True,timeout=5);return read_image(path)
  frozen_rect=state()["viewport"]
  def roi(image):
   assert state()["viewport"]==frozen_rect,"Viewport changed during comparison"
   x0,y0,x1,y1=[int(v)for v in state()["viewport"]];return image[y0:y1,x0:x1]
  base=take("baseline");base_roi=roi(base);assert base_roi.size>0 and np.any(base_roi[:,:,:3]!=0),"Black startup cannot prove empty viewport"
  def action(nonce,name,held=False):
   previous=state();request=control/"request.tmp";request.write_text(json.dumps(dict(nonce=nonce,action=name)));request.rename(control/"request.json")
   first=wait(lambda:(v if(v:=state())and v["nonce"]==nonce else None))
   # The nonce is observed before this frame submits. Only acknowledgment of a
   # presentation serial strictly beyond that observation can admit capture.
   value=first if held else wait(lambda:(v if(v:=state())and v["nonce"]==nonce and v["observation"]["completed_present_serial"]>first["observation"]["submitted_serial"] else None))
   assert value["observation"]["prepare_error"]is None or held
   if held:
    time.sleep(.1);assert state()["observation"]["submitted_serial"]==value["observation"]["submitted_serial"]
   image=take(str(nonce)+"-"+name);rows.append(dict(name=name,state=state(),image_sha256=hashlib.sha256((out/(str(nonce)+"-"+name+".png")).read_bytes()).hexdigest()));return image
  cold=state()["observation"]["world"]
  def image_work(v):return tuple(v[k]for k in ("image_hashes","image_decodes","image_uploads","sampled_entries","bc7_uploads"))
  camera=action(1,"camera");assert np.any(roi(camera)!=base_roi),"Camera turn did not produce presented receiver pixels"
  moved=state()["observation"]["world"];assert image_work(moved)==image_work(cold)and moved["base_builds"]==cold["base_builds"],"Camera rebuilt mesh/image cache"
  original=action(2,"original");assert np.array_equal(roi(original),base_roi),"Camera restore changed receiver image"
  hide=action(3,"hide");assert state()["hidden"]and np.any(roi(hide)!=base_roi),"Hidden receiver stayed visible"
  restore=action(4,"restore");assert not state()["hidden"]and np.array_equal(roi(restore),base_roi),"Restore failed exact pixels"
  before_refusal=state()["observation"]["world"]
  refused=action(5,"refuse",True);assert state()["observation"]["prepare_error"] and np.array_equal(refused,restore),"Refused frame changed actual presentation"
  assert image_work(state()["observation"]["world"])==image_work(before_refusal)
  retry=action(6,"retry");assert not state()["held"]and np.any(roi(retry)!=base_roi),"Retry failed receiver publication"
  restored=action(7,"original");assert np.array_equal(roi(restored),base_roi),"Final receiver restore changed pixels"
  before_resize=state()["observation"]["world"];lib.XResizeWindow(d,window,900,650);lib.XFlush(d)
  first_resize=wait(lambda:(v if(v:=state())and v["screen_size"]==[900.0,650.0]else None))
  resized=wait(lambda:(v if(v:=state())and v["screen_size"]==[900.0,650.0]and v["observation"]["completed_present_serial"]>first_resize["observation"]["submitted_serial"]else None))
  resized_image=take("resize");assert np.any(resized_image[:,:,:3]),"Resized application black"
  after_resize=resized["observation"]["world"];assert image_work(after_resize)==image_work(before_resize)and after_resize["base_builds"]==before_resize["base_builds"]and after_resize["vertex_uploads"]==before_resize["vertex_uploads"],"Resize rebuilt immutable receiver/image data"
  rows.append(dict(name="resize",state=resized,image_sha256=hashlib.sha256((out/"resize.png").read_bytes()).hexdigest()))
  request=control/"request.tmp";request.write_text(json.dumps(dict(nonce=8,action="close")));request.rename(control/"request.json")
  event=XEvent();event.client.type=33;event.client.send_event=1;event.client.display=d;event.client.window=window;event.client.message_type=lib.XInternAtom(d,b"WM_PROTOCOLS",0);event.client.format=32;event.client.data.l[0]=lib.XInternAtom(d,b"WM_DELETE_WINDOW",0);assert lib.XSendEvent(d,window,0,0,C.byref(event));lib.XFlush(d);assert p.wait(timeout=5)==0
 log=(out/"runtime.log").read_text();assert "committed_bytes=0"in log
 report=dict(source_sha=args.source,binary_sha256=hashlib.sha256(Path(args.binary).read_bytes()).hexdigest(),rows=rows,passed=True,checker_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),scope="Actual IngeTrazoApp/render_viewport camera/hide/restore/budget-refusal/retry, owned submitted/present acknowledgment and image cache reuse; direct source image proof separately retained")
 (out/"result.json").write_text(json.dumps(report,indent=2)+"\n");print("PASS actual billboard application camera/hide/restore/refusal/retry")
except BaseException as error:
 (out/"failure.json").write_text(json.dumps(dict(source_sha=args.source,error=repr(error),rows=rows,last_state=state()),indent=2)+"\n");raise
finally:
 if p is not None:stop_owned_helper(p,out/"app-cleanup.json",timeout=5)
 if d:lib.XCloseDisplay(d)
 stop_owned_helper(xvfb,out/"xvfb-cleanup.json",timeout=5);xvfb_log.close()
