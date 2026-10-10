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
 if not original:subprocess.run(["cargo","build","--locked","-p","ingetrazo-gui","--example","center_tracking_native_fixture"],check=True)
 native_binary=Path(os.environ.get("CARGO_TARGET_DIR","target"))/"debug/examples/center_tracking_native_fixture"
 mode=os.environ.get("INGETRAZO_CENTER_MODE","curve")
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
   log=open(directory/"runtime.log","w");command=[sys.executable,"scripts/original_center_ui_fixture.py",str(directory.resolve())] if original else [str(native_binary.resolve()),str(directory.resolve())];app=subprocess.Popen(command,env=env,stdout=log,stderr=log)
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
   capture("before");began=time.monotonic();move(initial["rim"])
   early=wait(lambda:state() if state()["kind"]=="Endpoint" and state()["time"]>initial["time"] else None)
   assert not early["memory"],early
   acquired=wait(lambda:state() if state()["memory"] else None)
   elapsed=time.monotonic()-began
   assert elapsed>=.2,"Center encouraged before the original 250ms dwell window"
   expected=initial["center"]
   if mode in ("curve","tiny"):
    assert close(acquired["memory"][-1],expected),acquired
   else:assert not close(acquired["memory"][-1],expected),acquired
   assert acquired["undo"]==0 and acquired["edges"]==initial["edges"]
   capture("dwelt-rim")
   approach_green=None
   if mode in ("curve","tiny","malformed"):
    approach=[initial["center_pixel"][0]+20,initial["center_pixel"][1]];move(approach);time.sleep(.1);capture("approach-hint")
    cx,cy=map(round,initial["center_pixel"])
    text=subprocess.check_output(["convert",str(directory/"approach-hint.png"),"-crop",f"17x17+{cx-8}+{cy-8}","-depth","8","txt:-"],text=True)
    colors=[tuple(map(int,values)) for values in re.findall(r"\((\d+),(\d+),(\d+)(?:,\d+)?\)",text)]
    approach_green=sum(r<100 and g>100 and b<150 for r,g,b in colors)
    if mode=="curve" and (original or os.environ.get("INGETRAZO_CENTER_EXPECT_HINT")=="1"):assert approach_green>5,"Center approach hint must materially render"
    if mode!="curve":assert approach_green==0,"Tiny/malformed approach center must stay quiet"
   move(initial["center_pixel"]);time.sleep(.1)
   centered=state()
   if mode!="curve":
    assert centered["kind"]!="Center",centered
    capture("quiet-center")
    report={"passed":True,"provenance":context(),"mode":mode,"early":early,"acquired":acquired,"centered":centered,"approach_green_pixels":approach_green,"elapsed_wall_seconds":elapsed,"scope":"Actual native Vulkan production UI pointer and frame clock; tiny dot quiet or malformed fit rejected, no geometry/undo mutation; llvmpipe Xvfb engineering backend."}
   else:
    centered=wait(lambda:state() if state()["kind"]=="Center" else None)
    assert close(centered["inference_point"],expected);capture("center-hint")
    click(initial["center_pixel"]);anchor=wait(lambda:state() if state()["anchor"] else None)
    assert close(anchor["anchor"],expected) and anchor["undo"]==0
    key(0xff1b);wait(lambda:state() if state()["anchor"] is None else None)
    ctrl_key(0x61);key(0x6d);move(initial["rim"]);click(initial["rim"]);move(initial["move_target"]);click(initial["move_target"])
    moved=wait(lambda:state() if state()["undo"]==1 else None)
    actual_delta=[moved["center"][i]-expected[i] for i in range(3)]
    assert .5<actual_delta[0]<1.5 and abs(actual_delta[1])<1e-6 and abs(actual_delta[2])<1e-6,moved
    move_parity=close(actual_delta,[1,0,0])
    pointer_quantized_move_matches=sum((a-b)**2 for a,b in zip(actual_delta,[1,0,0]))**.5<.01
    assert pointer_quantized_move_matches,actual_delta
    key(0x6c)
    # Move ends at its new rim. Original hover is event-driven: create actual
    # pointer motion after switching tool rather than warping to the same pixel.
    move([moved["rim"][0]+20,moved["rim"][1]+20]);move(moved["rim"]);wait(lambda:state() if state()["memory"] else None);move(moved["center_pixel"])
    moved_hint=wait(lambda:state() if state()["kind"]=="Center" else None)
    assert close(moved_hint["inference_point"],moved["center"]);capture("moved-center")
    ctrl_key(0x61);key(0xffff)
    move([moved["center_pixel"][0]+20,moved["center_pixel"][1]+20]);move(moved["center_pixel"])
    deleted=wait(lambda:state() if state()["edges"]==0 and state()["kind"]!="Center" else None)
    assert deleted["undo"]==2;capture("deleted-center")
    report={"passed":True,"provenance":context(),"early":early,"acquired":acquired,"centered":centered,"anchor":anchor,"moved":moved,"moved_hint":moved_hint,"deleted":deleted,"requested_move_delta":[1,0,0],"actual_move_delta":actual_delta,"move_coordinate_parity":move_parity,"pointer_quantized_move_matches":pointer_quantized_move_matches,"approach_green_pixels":approach_green,"approach_hint_visual_parity_open":approach_green<=5,"elapsed_wall_seconds":elapsed,"scope":"Native ash Vulkan full application UI, XTEST real pointer/dwell/click/Move/CtrlA/Delete, named center and Line anchor, movement/deletion cache, no extra preview undo. llvmpipe isolated Xvfb engineering backend, hardware/independent original UI pending."}
   report["executable_sha256"]=hashlib.sha256((Path("scripts/original_center_ui_fixture.py") if original else native_binary).read_bytes()).hexdigest()
   if original:report["scope"]="Actual original Qt/OpenGL Viewport with XTEST pointer and real QTimer dwell; standalone tool/select shortcuts wired to original public actions, Move/Delete original geometry/history, software GL Xvfb."
   Path("docs/audits/center-tracking-"+("original-" if original else "native-")+mode+suffix+"-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print("Actual center gesture PASS",mode,"original" if original else "native")
  finally:
   if app:
    app.terminate()
    try:app.wait(timeout=5)
    except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
   server.terminate()
   try:server.communicate(timeout=5)
   except subprocess.TimeoutExpired:server.kill();server.communicate(timeout=5)
if __name__=="__main__":main()
