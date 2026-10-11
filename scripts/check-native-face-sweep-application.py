#!/usr/bin/env python3
"""Disposable native X11 Vulkan startup/resize evidence; never touches desktop."""
import ctypes as C, os, subprocess, time, json, hashlib, select
from pathlib import Path
import argparse
from owned_process_cleanup import stop_owned_helper
parser=argparse.ArgumentParser();parser.add_argument("binary");parser.add_argument("output");parser.add_argument("--source-dir",required=True);parser.add_argument("--case",default="ordinary-prism");parser.add_argument("--source",required=True);parser.add_argument("--source-frames",action="store_true");parser.add_argument("--source-phases",default="base,preview,offset,cancel");args=parser.parse_args();assert len(args.source)==40 and all(c in "0123456789abcdef" for c in args.source)
out=Path(args.output);out.mkdir(parents=True,exist_ok=True)
source_rows=json.loads((Path(args.source_dir)/"original.json").read_text())["cases"];recipe=next(row for row in source_rows if row["name"]==args.case)
expected_distance=sum((recipe["anchor"][j]+recipe["normal"][j]*.6-recipe["anchor"][j])*recipe["normal"][j] for j in range(3))
xvfb=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1400x1000x24"],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
import atexit
atexit.register(lambda:stop_owned_helper(xvfb,out/"xvfb-setup-cleanup.json",timeout=5) if xvfb.poll() is None else None)
assert select.select([xvfb.stdout],[],[],5)[0],"Owned Xvfb startup timeout"
display=":"+xvfb.stdout.readline().strip();control=out/"control";control.mkdir(exist_ok=True);assert not (control/"state.json").exists() and not (control/"request.json").exists(),"Fresh control directory required"
size=(800,600) if args.source_frames else (1000,700)
env=dict(os.environ,DISPLAY=display,XDG_CONFIG_HOME=str(out/"config"),INGETRAZO_VULKAN_WORLD="1",INGETRAZO_SWEEP_SOURCE=str(Path(args.source_dir).resolve()),INGETRAZO_SWEEP_CASE=args.case,INGETRAZO_SWEEP_CONTROL=str(control))

if args.source_frames:env["INGETRAZO_SWEEP_SOURCE_FRAME"]="1"
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
 try:
  value=json.loads((control/"state.json").read_text());value.update(value["gpu"]);return value
 except FileNotFoundError:return None
def read_image(path):
 image=QImage(str(path));assert not image.isNull() and (image.width(),image.height())==size,"Missing/unreadable/wrong-extent PNG"
 image=image.convertToFormat(QImage.Format_RGBA8888)
 return np.frombuffer(image.constBits(),np.uint8).reshape(size[1],image.bytesPerLine())[:,:size[0]*4].reshape(size[1],size[0],4).copy()
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
  lib.XFree(children);assert len(matches)==1,f"Owned process window identity absent/ambiguous: {observed}";window=matches[0];before_resize=state()["submitted_serial"];lib.XResizeWindow(d,window,*size);lib.XFlush(d);wait(lambda:state()["screen_size"]==list(map(float,size)) and state()["completed_present_serial"]>before_resize)
  def take(name):
   path=out/(name+".png");subprocess.run(["import","-display",display,"-window",str(window),str(path)],check=True,timeout=5);return read_image(path)
  frozen_rect=state()["viewport"]
  def roi(image):
   assert state()["viewport"]==frozen_rect,"Viewport changed during comparison"
   x0,y0,x1,y1=[int(v)for v in state()["viewport"]];return image[y0:y1,x0:x1]
  base=take("baseline");base_roi=roi(base);assert base_roi.size and np.any(base_roi[:,:,:3]!=0)
  assert state()["label"] is None and not state()["dirty"]
  def action(nonce,name,refusal=False,**extra):
   request=control/"request.tmp";request.write_text(json.dumps(dict(nonce=nonce,action=name,**extra)));request.rename(control/"request.json")
   first=wait(lambda:(v if(v:=state())and v["nonce"]==nonce else None))
   if refusal:
    value=wait(lambda:(v if(v:=state())and v["nonce"]==nonce and v["prepare_error"]else None))
    time.sleep(.1);assert state()["submitted_serial"]==value["submitted_serial"]
   else:
    value=wait(lambda:(v if(v:=state())and v["nonce"]==nonce and not v["prepare_error"] and v["completed_present_serial"]>first["submitted_serial"] else None))
   image=take(str(nonce)+"-"+name);rows.append(dict(name=name,state=state(),image_sha256=hashlib.sha256((out/(str(nonce)+"-"+name+".png")).read_bytes()).hexdigest()));return image
  if args.source_frames:
   phases=set(args.source_phases.split(","));assert phases and phases<={"base","preview","offset","cancel"}
   # The frozen original observer never cancels acquisition: its base/cancel
   # phases set extrusion=0 while the length label remains active. Actual
   # cancellation is tested independently in the full application workflow.
   frames={"base":action(1,"acquire")}
   if "preview" in phases:frames["preview"]=action(2,"drag",distance=recipe["frames"][1]["distance"])
   if "offset" in phases:frames["offset"]=action(3,"drag",distance=recipe["frames"][2]["distance"])
   if "cancel" in phases:frames["cancel"]=action(4,"drag",distance=0.)
   comparisons=[]
   for reference in recipe["frames"]:
    if reference["phase"] not in phases:continue
    source_path=Path(args.source_dir)/reference["file"]
    assert hashlib.sha256(source_path.read_bytes()).hexdigest()==reference["sha256"],"Frozen source frame changed"
    source=read_image(source_path);native=frames[reference["phase"]]
    delta=np.abs(source.astype(np.int16)-native.astype(np.int16));mask=np.any(delta!=0,axis=2)
    comparisons.append(dict(phase=reference["phase"],source_sha256=reference["sha256"],different_pixels=int(mask.sum()),maximum=int(delta.max()),exact=not bool(mask.any())))
   (out/"source-comparison.json").write_text(json.dumps(dict(case=args.case,extent=size,comparisons=comparisons,passed=all(c["exact"]for c in comparisons)),indent=2)+"\n")
   action(5,"cancel");action(6,"save")
  else:
   acquired=action(1,"acquire");assert state()["label"] is not None and state()["label"]["distance"]==0 and np.any(roi(acquired)!=base_roi)
   shown=action(2,"drag",distance=.6);assert state()["label"]["distance"]==abs(expected_distance) and state()["world"]["preview_bytes"]>0 and np.any(roi(shown)!=roi(acquired))
   warm=dict(state()["world"]);serial=state()["submitted_serial"];wait(lambda:state()["completed_present_serial"]>serial+3)
   for key in ["base_uploads","preview_uploads","texture_builds","texture_uploads","sampled_entries"]:assert state()["world"][key]==warm[key],f"Warm work changed {key}"
   offset=action(3,"drag",distance=.9);assert state()["world"]["base_uploads"]==warm["base_uploads"] and state()["world"]["preview_uploads"]==warm["preview_uploads"]+1
   assert np.any(roi(offset)!=roi(shown))
   before=state();refused=action(4,"refuse",True,distance=1.2)
   assert np.array_equal(refused,offset),"Refusal changed previous full image"
   assert state()["committed_bytes"]==before["committed_bytes"] and state()["world"]==before["world"]
   retry=action(5,"retry");assert np.any(roi(retry)!=roi(offset))
   typed=action(6,"typed",text="600mm");assert np.any(roi(typed)!=roi(retry))
   before=state();font_refused=action(7,"refuse-font",True,text="123456789mm")
   assert "font" in state()["prepare_error"].lower()
   assert np.array_equal(font_refused,typed) and state()["committed_bytes"]==before["committed_bytes"] and state()["world"]==before["world"]
   assert state()["old_label_owner_alive"],"Font refusal dropped old complete label owner"
   font_retry=action(8,"retry");assert np.any(roi(font_retry)!=roi(typed))
   wait(lambda:state()["old_label_owner_alive"] is False)
   before=state();cad_refused=action(9,"refuse-cad",True,text="987654321mm")
   assert "budget" in state()["prepare_error"].lower()
   assert np.array_equal(cad_refused,font_retry) and state()["committed_bytes"]==before["committed_bytes"] and state()["world"]==before["world"]
   assert state()["old_label_owner_alive"],"CAD refusal dropped old complete label owner"
   cad_retry=action(10,"retry");assert np.any(roi(cad_retry)!=roi(font_retry))
   wait(lambda:state()["old_label_owner_alive"] is False)
   cancelled=action(11,"cancel");assert state()["label"] is None and np.array_equal(roi(cancelled),base_roi)
   before=state()["submitted_serial"];lib.XResizeWindow(d,window,900,650);lib.XFlush(d);wait(lambda:state()["screen_size"]==[900.0,650.0] and state()["completed_present_serial"]>before)
   before=state()["submitted_serial"];lib.XResizeWindow(d,window,1000,700);lib.XFlush(d);wait(lambda:state()["screen_size"]==[1000.0,700.0] and state()["completed_present_serial"]>before)
   assert np.array_equal(roi(take("resize-restored")),base_roi)
   action(12,"acquire");action(13,"drag",distance=.6);committed=action(14,"commit",text="0.6")
   assert state()["dirty"] and state()["label"] is None and np.any(roi(committed)!=base_roi)
   undone=action(15,"undo");assert np.array_equal(roi(undone),base_roi)
   action(16,"save");assert not state()["dirty"] and (control/"saved.igz").exists()
  event=XEvent();event.client.type=33;event.client.send_event=1;event.client.display=d;event.client.window=window;event.client.message_type=lib.XInternAtom(d,b"WM_PROTOCOLS",0);event.client.format=32;event.client.data.l[0]=lib.XInternAtom(d,b"WM_DELETE_WINDOW",0);assert lib.XSendEvent(d,window,0,0,C.byref(event));lib.XFlush(d);assert p.wait(timeout=5)==0
 log=(out/"runtime.log").read_text();assert "committed_bytes=0"in log
 report=dict(source_sha=args.source,binary_sha256=hashlib.sha256(Path(args.binary).read_bytes()).hexdigest(),rows=rows,passed=True,scope="Viewport-only source frame comparison" if args.source_frames else "Actual App and Tool acquisition/move/VCB dispatcher, native preview+label, offset-only/warm uploads, full-frame refusal/retry/cancel/resize and Undo. Control invokes production methods, not mouse event proof; source whole-image comparison separate")
 (out/"result.json").write_text(json.dumps(report,indent=2)+"\n");print("PASS actual application FaceSweep workflow")
except BaseException as error:
 (out/"failure.json").write_text(json.dumps(dict(source_sha=args.source,error=repr(error),rows=rows,last_state=state()),indent=2)+"\n");raise
finally:
 if p is not None:stop_owned_helper(p,out/"app-cleanup.json",timeout=5)
 lib.XCloseDisplay(d);stop_owned_helper(xvfb,out/"xvfb-cleanup.json",timeout=5)
