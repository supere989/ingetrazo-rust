#!/usr/bin/env python3
"""Actual native pointer/key events through a rendered production viewport."""
import sys,hashlib,ctypes as C,ctypes.util,json,os,subprocess,tempfile,time,select,struct,zlib
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
def main():
 source=Path(sys.argv[1]).resolve()
 subprocess.run(["cargo","build","--locked","-p","ingetrazo-gui","--example","composer_history_acceptance_fixture"],cwd=source,check=True)
 directory=Path("docs/audits/composer-history-render");directory.mkdir(exist_ok=True)
 for p in directory.iterdir():
  if p.is_file():p.unlink()
 with tempfile.TemporaryDirectory(prefix="ingetrazo-offset-x11-") as t:
  server=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1200x900x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  app=None
  try:
   assert select.select([server.stdout],[],[],10)[0];display=server.stdout.readline().strip();assert display.isdigit()
   env=os.environ.copy();env["DISPLAY"]=":"+display;env["XDG_RUNTIME_DIR"]=t;env["XDG_CONFIG_HOME"]=t+"/config";env["XDG_CACHE_HOME"]=t+"/cache";env["LIBGL_ALWAYS_SOFTWARE"]="1";env.pop("WAYLAND_DISPLAY",None)
   log=open(directory/"runtime.log","w");app=subprocess.Popen([str(source/"target/debug/examples/composer_history_acceptance_fixture"),str(directory.resolve())],env=env,stdout=log,stderr=log)
   state=lambda:json.loads((directory/"state.json").read_text())
   initial=wait(lambda:state() if state()["model_faces"]==1 else None)
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
    (directory/"capture.txt").write_text(name);wait(lambda:(directory/(name+".png")).is_file())
   events=[]
   def record(name):
    value=state();events.append({"case":name,"state":value});capture(name);return value
   def key(symbol):
    x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint;code=x.XKeysymToKeycode(d,symbol);event(2,code,None,None);event(3,code,None,None)
   x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint
   def control(symbol):
    code=x.XKeysymToKeycode(d,0xffe3);event(2,code,None,None);key(symbol);event(3,code,None,None)
   def begin_drag():
    point=state()["click"];click(point);move(point);event(4,1,None,None);move([point[0]+10,point[1]+5]);move([point[0]+70,point[1]+35]);return point
   def drag():
    begin_drag();event(5,1,None,None);return wait(lambda:state()if not state()["pending"]else None)
   before=record("initial");dragged=drag();assert dragged["sheets"][0]["rect"]!=before["sheets"][0]["rect"];assert dragged["sheets"][0]["undo"]==1 and dragged["model_undo"]==0;record("dragged")
   control(ord("z"));undone=wait(lambda:state()if state()["sheets"][0]["undo"]==0 else None);assert undone["sheets"][0]["rect"]==before["sheets"][0]["rect"];record("undone")
   control(ord("y"));redone=wait(lambda:state()if state()["sheets"][0]["undo"]==1 else None);assert redone["sheets"][0]["rect"]==dragged["sheets"][0]["rect"];record("redone")
   begin_drag();wait(lambda:state()["pending"]);key(0xff1b);event(5,1,None,None);cancelled=wait(lambda:state()if not state()["pending"]else None);assert cancelled["sheets"][0]["rect"]==redone["sheets"][0]["rect"] and cancelled["sheets"][0]["undo"]==1;record("cancelled")
   click(state()["buttons"]["Model"]);wait(lambda:state()["active"]is None);click(state()["buttons"]["Extrude"]);model=wait(lambda:state()if state()["model_faces"]>1 else None);assert model["model_undo"]==1;record("model-extruded")
   click(state()["buttons"]["Sheet 1"]);wait(lambda:state()["active"]==0);edited=drag();assert edited["sheets"][0]["undo"]==2;record("sheet-after-model")
   sheets=edited["sheets"];click(state()["buttons"]["Model"]);wait(lambda:state()["active"]is None);control(ord("z"));restored=wait(lambda:state()if state()["model_faces"]==1 else None);assert restored["sheets"]==sheets;record("model-undone-sheet-retained")
   click(state()["buttons"]["Sheet 1"]);wait(lambda:state()["active"]==0);begin_drag();pending=wait(lambda:state()if state()["pending"]else None);sheet_two=pending["sheets"][1];key(0xffbf);switched=wait(lambda:state()if state()["active"]==1 and not state()["pending"]else None);assert switched["drag"]is None;move([650,500]);event(5,1,None,None);after=state();assert after["sheets"][1]==sheet_two and after["sheets"][0]["undo"]==3;record("switched-mid-drag")
   report={"source_revision":subprocess.check_output(["git","rev-parse","HEAD"],cwd=source,text=True).strip(),"native_binary_sha256":hashlib.sha256((source/"target/debug/examples/composer_history_acceptance_fixture").read_bytes()).hexdigest(),"passed":True,"cases":events,"scope":"XTEST native gestures through production render_sheet_composer in an instrumented native eframe host. Drag one history entry, keyboard undo/redo routing to production app methods, Escape rollback, actual production PushPull dispatcher model commit, later sheet edit retained across model undo, F2 sheet switch while pointer held retires drag without mutating second sheet. Host navigation buttons/key routing instrumented; full application menus/shortcut dispatch and cross-platform display remain separate."}
   Path("docs/audits/composer-history-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
  finally:
   if app:app.terminate();app.wait(timeout=5)
   server.terminate();server.communicate(timeout=5)
if __name__=="__main__":main()
