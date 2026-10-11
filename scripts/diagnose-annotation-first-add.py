#!/usr/bin/env python3
"""Disposable native X11 Vulkan startup/resize evidence; never touches desktop."""
import ctypes as C, os, subprocess, time, json, hashlib, select
from pathlib import Path
import argparse
from owned_process_cleanup import stop_owned_helper
parser=argparse.ArgumentParser();parser.add_argument("binary");parser.add_argument("output");parser.add_argument("--source",required=True);args=parser.parse_args();assert len(args.source)==40 and all(c in "0123456789abcdef" for c in args.source)
out=Path(args.output);out.mkdir(parents=True,exist_ok=True)
xvfb=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1400x1000x24"],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
assert select.select([xvfb.stdout],[],[],5)[0],"Owned Xvfb startup timeout"
display=":"+xvfb.stdout.readline().strip();control=out/"control";control.mkdir(exist_ok=True)
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
rows=[];p=None
try:
 with(out/"runtime.log").open("w")as log:
  p=subprocess.Popen([args.binary],env=env,stdout=log,stderr=log)
  wait(lambda:state()and state()["viewport"]and state()["submitted_serial"]>1)
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
  lib.XFree(children);assert len(matches)==1,f"Owned process window identity absent/ambiguous: {observed}";window=matches[0];before_resize=state()["submitted_serial"];lib.XResizeWindow(d,window,1000,700);lib.XFlush(d);wait(lambda:state()["screen_size"]==[1000.0,700.0] and state()["submitted_serial"]>before_resize)
  def take(name):
   path=out/(name+".png");subprocess.run(["import","-display",display,"-window",str(window),str(path)],check=True,timeout=5);return read_image(path)
  frozen_rect=state()["viewport"]
  def roi(image):
   assert state()["viewport"]==frozen_rect,"Viewport changed during comparison"
   x0,y0,x1,y1=[int(v)for v in state()["viewport"]];return image[y0:y1,x0:x1]
  base=take("baseline");base_roi=roi(base);assert base_roi.size>0 and np.any(base_roi[:,:,:3]!=0),"Black startup cannot prove empty viewport"
  def action(nonce,name,held=False):
   previous=state();request=control/"request.tmp";request.write_text(json.dumps(dict(nonce=nonce,action=name)));request.rename(control/"request.json")
   value=wait(lambda:(v if(v:=state())and v["nonce"]==nonce and(held or v["submitted_serial"]>previous["submitted_serial"])else None))
   if held:
    time.sleep(.1);assert state()["submitted_serial"]==value["submitted_serial"]
   image=take(str(nonce)+"-"+name);rows.append(dict(name=name,state=state(),image_sha256=hashlib.sha256((out/(str(nonce)+"-"+name+".png")).read_bytes()).hexdigest()));return image
  add=action(1,"add");immediate_changed=int(np.count_nonzero(np.any(roi(add)!=base_roi,axis=2)))
  observed=state()['submitted_serial'];settled=[]
  for advance in [3,8]:
   wait(lambda:state()['submitted_serial']>=observed+advance)
   image=take('settled-'+str(advance));settled.append(dict(advance=advance,state=state(),changed_pixels=int(np.count_nonzero(np.any(roi(image)!=base_roi,axis=2)))))
  (out/'diagnostic.json').write_text(json.dumps(dict(immediate_changed_pixels=immediate_changed,settled=settled,scope='Same unchanged first-add scene; captures after measured serial advancement, no gesture retries; serial is pre-submit observation rather than presentation acknowledgment'),indent=2)+'\n')
  event=XEvent();event.client.type=33;event.client.send_event=1;event.client.display=d;event.client.window=window;event.client.message_type=lib.XInternAtom(d,b"WM_PROTOCOLS",0);event.client.format=32;event.client.data.l[0]=lib.XInternAtom(d,b"WM_DELETE_WINDOW",0);assert lib.XSendEvent(d,window,0,0,C.byref(event));lib.XFlush(d);assert p.wait(timeout=5)==0
 log=(out/"runtime.log").read_text();assert "committed_bytes=0"in log
 report=dict(immediate_first_add_changed_pixels=immediate_changed,settled=settled,source_sha=args.source,binary_sha256=hashlib.sha256(Path(args.binary).read_bytes()).hexdigest(),rows=rows,passed=immediate_changed>0,scope="First-add scheduling diagnosis only; original immediate failure remains retained")
 (out/"result.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(dict(immediate_changed_pixels=immediate_changed,settled=settled)))
except BaseException as error:
 (out/"failure.json").write_text(json.dumps(dict(source_sha=args.source,error=repr(error),rows=rows,last_state=state()),indent=2)+"\n");raise
finally:
 if p is not None:stop_owned_helper(p,out/"app-cleanup.json",timeout=5)
 lib.XCloseDisplay(d);stop_owned_helper(xvfb,out/"xvfb-cleanup.json",timeout=5)
