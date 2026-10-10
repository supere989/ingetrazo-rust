#!/usr/bin/env python3
"""Actual native pointer/key events through a rendered production viewport."""
import sys,hashlib,ctypes as C,ctypes.util,json,os,subprocess,tempfile,time,select,struct,zlib
from pathlib import Path
from acceptance_context import context
from PySide6.QtGui import QImage
def wait(fn,seconds=20):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  try:
   v=fn()
   if v:return v
  except (OSError,ValueError):pass
  time.sleep(.05)
 raise AssertionError("Timed out awaiting rendered event/state")
def main():
 original=os.environ.get("INGETRAZO_PREVIEW_ORIGINAL")=="1"
 source=Path(sys.argv[1]).resolve()
 subprocess.run(["cargo","build","--locked","-p","ingetrazo-gui","--example","composer_preview_native_fixture"],cwd=source,check=True)
 directory=Path("docs/audits/composer-direct-preview-"+("original" if original else "native"));directory.mkdir(exist_ok=True)
 for p in directory.iterdir():
  if p.is_file():p.unlink()
 with tempfile.TemporaryDirectory(prefix="ingetrazo-offset-x11-") as t:
  server=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1200x900x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  app=None
  try:
   assert select.select([server.stdout],[],[],10)[0];display=server.stdout.readline().strip();assert display.isdigit()
   env=os.environ.copy();env["DISPLAY"]=":"+display;env["XDG_RUNTIME_DIR"]=t;env["XDG_CONFIG_HOME"]=t+"/config";env["XDG_CACHE_HOME"]=t+"/cache";env["VK_DRIVER_FILES"]="/usr/share/vulkan/icd.d/lvp_icd.json";env["INGETRAZO_VULKAN_DEVICE"]="llvmpipe";env["INGETRAZO_PREVIEW_OUT"]=str(directory.resolve());env.pop("WAYLAND_DISPLAY",None)
   log=open(directory/"runtime.log","w");app=subprocess.Popen([sys.executable,"scripts/original_composer_preview_fixture.py"] if original else [str(source/"target/debug/examples/composer_preview_native_fixture"),str(directory.resolve())],env=env,stdout=log,stderr=log)
   state=lambda:json.loads((directory/"state.json").read_text())
   initial=wait(lambda:state() if len(state()["items"])==0 else None)
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
   def move(point):
    x.XWarpPointer(d,0,window,0,0,0,0,round(point[0]),round(point[1]));x.XFlush(d);time.sleep(.2)
   xt=C.CDLL(ctypes.util.find_library("Xtst"));xt.XTestFakeButtonEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong];xt.XTestFakeKeyEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
   def event(kind,code,point,mask):
    if kind in (4,5):assert xt.XTestFakeButtonEvent(d,code,kind==4,0)
    else:assert xt.XTestFakeKeyEvent(d,code,kind==2,0)
    x.XFlush(d);time.sleep(.15)
   def click(point):
    move(point);event(4,1,point,4);event(5,1,point,8)
   def capture(name):
    target=directory/(name+".png")
    def presented():
     subprocess.run(["import","-display",env["DISPLAY"],"-window",str(window),str(target)],check=True)
     im=QImage(str(target));assert not im.isNull() and im.width()>0 and im.height()>0
     paper=state()["paper"];c=im.pixelColor(round(paper[2]-40),round(paper[3]-40))
     return min(c.red(),c.green(),c.blue())>240
    wait(presented)
   events=[]
   def record(name):
    value=state();events.append({"case":name,"state":value});capture(name);return value
   def key(symbol):
    x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint;code=x.XKeysymToKeycode(d,symbol);event(2,code,None,None);event(3,code,None,None)
   x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint
   def control(symbol):
    code=x.XKeysymToKeycode(d,0xffe3);event(2,code,None,None);key(symbol);event(3,code,None,None)
   before=record("before");click(state()["button"]);paper=state()["paper"];start=[paper[0]+100,paper[1]+120];scale=(paper[2]-paper[0])/state()["paper_mm"][0];end=[start[0]+50*scale,start[1]+35*scale]
   move(start);event(4,1,None,None);move(end);preview=wait(lambda:state() if state()["start"] is not None and (state()["preview_alive"] if original else state()["metrics"]["uploads"]>0) else None)
   assert preview["undo"]==0 and not preview["items"];record("placement-preview")
   time.sleep(.35);warm=record("warm-preview")
   if not original:assert warm["metrics"]["uploads"]==preview["metrics"]["uploads"] and warm["metrics"]["cache_hits"]>preview["metrics"]["cache_hits"]
   key(0xff1b);event(5,1,None,None);cancel=wait(lambda:state() if state()["start"] is None and (original or state()["tool"] is None) else None);assert not cancel["items"] and cancel["undo"]==0;record("cancelled")
   click(state()["button"]);move(start);event(4,1,None,None);move(end);wait(lambda:state()["start"] is not None);event(5,1,None,None);committed=wait(lambda:state() if len(state()["items"])==1 and state()["undo"]==1 else None);record("committed")
   control(ord("z"));undone=wait(lambda:state() if not state()["items"] and state()["undo"]==0 else None);record("undone")
   if not original:
    control(ord("y"));redone=wait(lambda:state() if len(state()["items"])==1 and state()["undo"]==1 else None);record("redone")
   report={"passed":True,"original":original,"start_pixel":start,"end_pixel":end,"requested_size_mm":[50,35],"source_redo_keyboard_open":original,"source_revision":subprocess.check_output(["git","rev-parse","HEAD"],cwd=source,text=True).strip(),"binary_sha256":(None if original else hashlib.sha256((source/"target/debug/examples/composer_preview_native_fixture").read_bytes()).hexdigest()),"reference_fixture_sha256":(hashlib.sha256((source/"scripts/original_composer_preview_fixture.py").read_bytes()).hexdigest() if original else None),"cases":events,"scope":("Original Qt actual toolbar placement/Escape/commit/Undo; Redo keyboard open." if original else "Actual XTEST placement/Escape/commit/UndoRedo through production composer direct CAD factory, software Vulkan; fixture toolbar/keyboard routes.")+" No whole-image or AA parity."}
   (directory/"verdict.json").write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
  finally:
   if app:
    app.terminate()
    try:app.wait(timeout=5)
    except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
   server.terminate()
   try:server.communicate(timeout=5)
   except subprocess.TimeoutExpired:server.kill();server.communicate(timeout=5)
if __name__=="__main__":main()
