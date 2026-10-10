#!/usr/bin/env python3
"""Actual native pointer/key events through a rendered production viewport."""
import ctypes as C,ctypes.util,json,os,sys,subprocess,tempfile,time,select,struct,zlib,re,hashlib
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

from png_capture import valid_png

def main():
 original=os.environ.get("INGETRAZO_CENTER_ORIGINAL")=="1"
 native_binary=(Path(os.environ.get("CARGO_TARGET_DIR","target"))/"debug/examples/center_tracking_native_fixture").resolve()
 if not original:subprocess.run(["cargo","build","--locked","-p","ingetrazo-gui","--example","center_tracking_native_fixture"],check=True)
 mode=os.environ.get("INGETRAZO_CENTER_MODE","curve")
 requested_delta=[float(os.environ.get("INGETRAZO_MOVE_DISTANCE","1")),0,0]
 suffix=os.environ.get("INGETRAZO_CENTER_RUN_SUFFIX","")
 directory=Path("docs/audits/center-tracking-"+("original-" if original else "native-")+mode+suffix);directory.mkdir(exist_ok=True)
 for p in directory.iterdir():
  if p.name=="state.json" or p.suffix==".png":p.unlink()
 with tempfile.TemporaryDirectory(prefix="ingetrazo-reference-x11-") as t:
  server=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1200x900x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  app=None
  try:
   assert select.select([server.stdout],[],[],10)[0];display=server.stdout.readline().strip();assert display.isdigit()
   env=os.environ.copy();env["DISPLAY"]=":"+display;env["XDG_RUNTIME_DIR"]=t;env["VK_DRIVER_FILES"]="/usr/share/vulkan/icd.d/lvp_icd.json";env["INGETRAZO_VULKAN_DEVICE"]="llvmpipe";env.pop("WAYLAND_DISPLAY",None)
   log=open(directory/"runtime.log","w");command=[sys.executable,"scripts/original_center_ui_fixture.py",str(directory.resolve())] if original else [str(native_binary),str(directory.resolve())];app=subprocess.Popen(command,env=env,stdout=log,stderr=log)
   state=lambda:json.loads((directory/"state.json").read_text())
   initial=wait(lambda:state() if state()["edges"]>0 else None)
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
   def move(point,pause=.05):
    x.XWarpPointer(d,0,window,0,0,0,0,round(point[0]),round(point[1]));x.XFlush(d);time.sleep(pause)
   xt=C.CDLL(ctypes.util.find_library("Xtst"));xt.XTestFakeButtonEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong];xt.XTestFakeKeyEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
   def event(kind,code,point,mask):
    if kind in (4,5):assert xt.XTestFakeButtonEvent(d,code,kind==4,0)
    else:assert xt.XTestFakeKeyEvent(d,code,kind==2,0)
    x.XFlush(d);time.sleep(.15)
   def click(point):
    move(point);event(4,1,point,4);event(5,1,point,8)
   def capture(name):
    subprocess.run(["import","-display",env["DISPLAY"],"-window",str(window),"-define","png:color-type=6",str(directory/(name+".png"))],check=True);assert valid_png(directory/(name+".png"))
   x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint
   def key(symbol):
    code=x.XKeysymToKeycode(d,symbol);event(2,code,None,1);event(3,code,None,2)
   def close(a,b):return sum((x-y)**2 for x,y in zip(a,b))**.5<1e-5
   def ctrl_key(symbol):
    control=x.XKeysymToKeycode(d,0xffe3);event(2,control,None,1);key(symbol);event(3,control,None,2)
   assert mode=="group"
   baseline_groups=initial["group_count"]
   capture("group-before");time.sleep(.3)
   move(initial["rim"]);click(initial["rim"]);move(initial["move_target"])
   live=wait(lambda:state() if state()["anchor"] and state()["undo"]==0 else None)
   assert sum((live["center"][i]-initial["center"][i]-requested_delta[i])**2 for i in range(3))**.5<.01,live
   capture("group-live-preview")
   cancelled=None
   if os.environ.get("INGETRAZO_GROUP_NESTED")=="1":
    key(0xff1b);cancelled=wait(lambda:state() if state()["anchor"] is None else None)
    assert cancelled["context_depth"]==1 and cancelled["undo"]==0 and close(cancelled["center"],initial["center"]),cancelled
    capture("nested-live-preview-cancel")
    key(0x6d);move(initial["rim"]);click(initial["rim"]);move(initial["move_target"])
    wait(lambda:state() if state()["anchor"] else None)
   control=x.XKeysymToKeycode(d,0xffe3);event(2,control,None,1);event(3,control,None,2)
   preview=wait(lambda:state() if state()["preview_lines"]>1 else None);capture("group-copy-preview")
   assert preview["undo"]==0 and close(preview["center"],initial["center"]),preview
   click(initial["move_target"])
   copied=wait(lambda:state() if state()["group_count"]==baseline_groups+1 and state()["undo"]==1 else None);capture("group-copy-commit")
   assert len(set(copied["group_ids"]))==baseline_groups+1
   def type_value(value):
    if not original:
     rr,xx,yy,w,h,border,depth=C.c_ulong(),C.c_int(),C.c_int(),C.c_uint(),C.c_uint(),C.c_uint(),C.c_uint()
     x.XGetGeometry(d,window,C.byref(rr),C.byref(xx),C.byref(yy),C.byref(w),C.byref(h),C.byref(border),C.byref(depth))
     click([w.value-145,h.value-12])
    for char in value:key(ord(char))
    key(0xff0d)
   type_value("3x200cm")
   spaced=wait(lambda:state() if state()["group_count"]==baseline_groups+3 else None);capture("group-spaced-array")
   assert spaced["undo"]==1 and len(set(spaced["group_ids"]))==baseline_groups+3
   starts=[spaced["group_centers"][0][0]-initial["center"][0]]+[p[0]-initial["center"][0] for p in spaced["group_centers"][baseline_groups:]]
   assert all(abs(a-b)<.01 for a,b in zip(starts,[0,2,4,6])),spaced
   type_value("2x400cm")
   retyped=wait(lambda:state() if state()["group_count"]==baseline_groups+2 else None);capture("group-array-retyped")
   assert retyped["undo"]==1
   starts=[retyped["group_centers"][0][0]-initial["center"][0]]+[p[0]-initial["center"][0] for p in retyped["group_centers"][baseline_groups:]]
   assert all(abs(a-b)<.01 for a,b in zip(starts,[0,4,8])),retyped
   ctrl_key(0x7a);undone=wait(lambda:state() if state()["group_count"]==baseline_groups and state()["undo"]==0 else None);capture("group-array-undo")
   assert undone["group_ids"]==initial["group_ids"] and close(undone["center"],initial["center"])
   if os.environ.get("INGETRAZO_GROUP_LINKED")=="1":
    for observed in [live,preview,copied,spaced,retyped,undone]:
     assert observed["group_ids"][1]==initial["group_ids"][1] and close(observed["group_centers"][1],initial["group_centers"][1]),observed
     if original:assert len(set(observed["group_mesh_identity"]))==1,observed
     else:
      assert observed["definitions"]==initial["definitions"],observed
      assert len(set(observed["group_definitions"]))==1,observed
   if os.environ.get("INGETRAZO_GROUP_NESTED")=="1":
    assert initial["context_depth"]==1
    for observed in [live,preview,copied,spaced,retyped,undone]:assert observed["context_depth"]==1,observed
    if not original:assert undone["definitions"]==initial["definitions"],undone
   report={"cancelled_preview":cancelled,"nested_local":os.environ.get("INGETRAZO_GROUP_NESTED")=="1","linked":os.environ.get("INGETRAZO_GROUP_LINKED")=="1","passed":True,"provenance":context(),"live":live,"copy_preview":preview,"copied":copied,"spaced":spaced,"retyped":retyped,"undone":undone,"scope":"Actual rendered XTEST Move group pointer grab/preview/Ctrl copy/commit then keyboard spacing retyping and Undo. Real native ash Vulkan or original Qt/OpenGL viewport, isolated software GPU. Ordinary or linked placed group (linked flag explicit); shared definition unchanged and untouched sibling invariant where linked. Nested editing, alpha/depth batch performance and general P08 remain open."}
   if os.environ.get("INGETRAZO_MOVE_COPY")=="1":report["proof_boundary"]="Connected24-edge curve copy and actual Ctrl/live-preview/Undo gesture; source unused vertex registry retention recorded separately, not complete Scene/model parity. Positive ghost image patch and quiet negative are bounded visual controls."
   report["checker_sha256"]=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
   report["executable_sha256"]=hashlib.sha256((Path("scripts/original_center_ui_fixture.py") if original else native_binary).read_bytes()).hexdigest()
   if False:report["scope"]="Actual original Qt/OpenGL Viewport with XTEST pointer and real QTimer dwell; standalone tool/select/Undo shortcuts wired to original public actions, software GL Xvfb. "+("Move live preview, three Ctrl press/held/release toggles, copy wireframe, 24-edge curve copy and Undo; arrays/othercopytypes separate." if os.environ.get("INGETRAZO_MOVE_COPY")=="1" else "Move/Delete original geometry/history, center acquisition and invalidation; broader Move workflow separate.")
   Path("docs/audits/center-tracking-"+("original-" if original else "native-")+mode+suffix+"-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print("Actual center gesture PASS",mode,"original" if original else "native")
  finally:
   if app:app.terminate();app.wait(timeout=5)
   server.terminate();server.communicate(timeout=5)
if __name__=="__main__":main()
