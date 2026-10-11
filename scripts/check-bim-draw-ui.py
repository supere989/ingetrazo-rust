#!/usr/bin/env python3
"""Owned XTEST delivery to production app BIM tray and viewport."""
import ctypes as C,ctypes.util,json,os,select,subprocess,tempfile,time,hashlib,re
from pathlib import Path
root=Path(__file__).resolve().parents[1];original=bool(os.environ.get("INGETRAZO_BIM_ORIGINAL"));out=root/("docs/audits/bim-draw/original-ui"if original else"docs/audits/bim-draw/ui");out.mkdir(parents=True,exist_ok=True)
exe=Path(os.environ["CARGO_TARGET_DIR"])/"debug/examples/bim_draw_ui_fixture"
def wait(f):
 end=time.monotonic()+15
 while time.monotonic()<end:
  try:
   v=f()
   if v:return v
  except (ValueError,OSError,KeyError):pass
  time.sleep(.03)
 raise AssertionError("BIM UI observation deadline")
(out/"state.json").unlink(missing_ok=True);rows=[]
with tempfile.TemporaryDirectory(prefix="bim-ui-")as runtime:
 server=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1400x1000x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);p=None
 try:
  assert select.select([server.stdout],[],[],10)[0];display=":"+server.stdout.readline().strip();env=dict(os.environ,DISPLAY=display,XDG_RUNTIME_DIR=runtime);env.pop("WAYLAND_DISPLAY",None)
  with(out/"runtime.log").open("w")as log:
   env["INGETRAZO_BIM_UI_OUT"]=str(out);env["QT_QPA_PLATFORM"]="xcb";p=subprocess.Popen([__import__("sys").executable,str(root/"scripts/bim-draw-original-ui.py")]if original else[str(exe),str(out)],env=env,stdout=log,stderr=log)
   x=C.CDLL(ctypes.util.find_library("X11"));x.XOpenDisplay.argtypes=[C.c_char_p];x.XOpenDisplay.restype=C.c_void_p;d=x.XOpenDisplay(display.encode());assert d
   x.XDefaultRootWindow.argtypes=[C.c_void_p];x.XDefaultRootWindow.restype=C.c_ulong;wr=x.XDefaultRootWindow(d)
   x.XQueryTree.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ulong)),C.POINTER(C.c_uint)];rr,pp,nn=C.c_ulong(),C.c_ulong(),C.c_uint();children=C.POINTER(C.c_ulong)()
   x.XFetchName.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_char_p)];x.XFetchName.restype=C.c_int
   x.XFree.argtypes=[C.c_void_p]
   x.XGetGeometry.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_uint),C.POINTER(C.c_uint),C.POINTER(C.c_uint),C.POINTER(C.c_uint)]
   class Attributes(C.Structure):
    _fields_=[(n,t)for n,t in [("x",C.c_int),("y",C.c_int),("width",C.c_int),("height",C.c_int),("border",C.c_int),("depth",C.c_int),("visual",C.c_void_p),("root",C.c_ulong),("class",C.c_int),("bit_gravity",C.c_int),("win_gravity",C.c_int),("backing_store",C.c_int),("backing_planes",C.c_ulong),("backing_pixel",C.c_ulong),("save_under",C.c_int),("colormap",C.c_ulong),("map_installed",C.c_int),("map_state",C.c_int),("all_event_masks",C.c_long),("your_event_mask",C.c_long),("do_not_propagate",C.c_long),("override_redirect",C.c_int),("screen",C.c_void_p)]]
   x.XGetWindowAttributes.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(Attributes)]
   def window():
    x.XQueryTree(d,wr,C.byref(rr),C.byref(pp),C.byref(children),C.byref(nn));ids=[children[i]for i in range(nn.value)];x.XFree(children)
    names=[]
    for wid in ids:
     name=C.c_char_p();x.XFetchName(d,wid,C.byref(name));text=(name.value or b"").decode(errors="replace");names.append([wid,text])
     if name:x.XFree(name)
     if "IngeTrazo"in text or "BIM actual draw"in text:
      rootwin=C.c_ulong();xx,yy=C.c_int(),C.c_int();ww,hh,border,depth=C.c_uint(),C.c_uint(),C.c_uint(),C.c_uint();x.XGetGeometry(d,wid,C.byref(rootwin),C.byref(xx),C.byref(yy),C.byref(ww),C.byref(hh),C.byref(border),C.byref(depth))
      attrs=Attributes();x.XGetWindowAttributes(d,wid,C.byref(attrs))
      if ww.value>=1000 and hh.value>=700 and attrs.map_state==2:return wid
    (out/"window-tree.json").write_text(json.dumps(names));return None
   state=lambda:json.loads((out/"state.json").read_text());wait(state);wid=state()["window_id"]if original else wait(window)
   x.XSetInputFocus.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_ulong];x.XFlush.argtypes=[C.c_void_p];x.XSetInputFocus(d,wid,1,0);x.XFlush(d)
   x.XWarpPointer.argtypes=[C.c_void_p,C.c_ulong,C.c_ulong,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_int,C.c_int];xt=C.CDLL(ctypes.util.find_library("Xtst"));xt.XTestFakeButtonEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
   def click(pt):
    x.XWarpPointer(d,0,wid,0,0,0,0,round(pt[0]),round(pt[1]));x.XFlush(d);time.sleep(.03)
    for down in (1,0):assert xt.XTestFakeButtonEvent(d,1,down,0);x.XFlush(d);time.sleep(.03)
   x.XStringToKeysym.argtypes=[C.c_char_p];x.XStringToKeysym.restype=C.c_ulong;x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint;xt.XTestFakeKeyEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
   def key(n,ctrl=False,shift=False):
    def event(k,v):assert xt.XTestFakeKeyEvent(d,x.XKeysymToKeycode(d,x.XStringToKeysym(k.encode())),v,0);x.XFlush(d)
    if ctrl:event("Control_L",1)
    if shift:event("Shift_L",1)
    event(n,1);event(n,0)
    if shift:event("Shift_L",0)
    if ctrl:event("Control_L",0)
   def record(label):
    rows.append({"label":label,"state":state()});subprocess.run(["import","-display",display,"-window",str(wid),str(out/(label+".png"))],check=True)
   record("initial");s=state();toggle=s.get("toggle",[s["viewport"][0]+s["viewport"][2]+24,205]);click(toggle);wait(lambda:state()["active"]);record("active")
   key("r");wait(lambda:state()["tool"]=="Rectangle");click(state()["points"][0]);click(state()["points"][1]);wait(lambda:len(state()["faces"])==1);assert state()["faces"][0]["attrs"]["ifc"]=={"class":"IfcWall","name":"Wall","id":1};assert state()["undo"]==1;record("first")
   s=state();click(s.get("name_point",[s["viewport"][0]+s["viewport"][2]+122,142]));key("a",True)
   for letter in "next":key(letter)
   key("Return")
   wait(lambda:state()["tag"]["name"]=="next");record("rearmed")
   click(state()["points"][2]);click(state()["points"][3]);wait(lambda:len(state()["faces"])==2);assert {f["attrs"]["ifc"]["id"]for f in state()["faces"]}=={1,2};assert state()["undo"]==2;assert next(f for f in state()["faces"]if f["attrs"]["ifc"]["id"]==2)["attrs"]["ifc"]["name"]=="next";record("second")
   key("z",True);wait(lambda:len(state()["faces"])==1);assert state()["active"];record("undo")
   if original:click(state()["edit_menu"]);wait(lambda:state()["redo_point"]);click(state()["redo_point"])
   else:key("z",True,True)
   wait(lambda:len(state()["faces"])==2);record("redo")
   click(toggle);wait(lambda:not state()["active"]);assert state()["tag"] is None;record("off");key("z",True);wait(lambda:len(state()["faces"])==1);assert state()["tag"]is None;record("off-undo")
   if original:click(state()["edit_menu"]);wait(lambda:state()["redo_point"]);click(state()["redo_point"])
   else:key("z",True,True)
   wait(lambda:len(state()["faces"])==2);assert state()["tag"]is None;record("off-redo")
 finally:
  if p and p.poll()is None:p.terminate();p.wait(timeout=10)
  server.terminate();server.wait(timeout=10)
(out/"report.json").write_text(json.dumps({"passed":True,"binary_sha256":hashlib.sha256(Path(__import__("sys").executable).read_bytes()if original else exe.read_bytes()).hexdigest(),"states":rows,"original":original,"original_source":subprocess.check_output(["git","-C","/work/ingetrazo","rev-parse","HEAD"],text=True).strip(),"scope":"Actual original MainWindow widget/History delivery"if original else"Actual production IngeTrazoApp update, BIM checkbox, Rectangle shortcut/pointer commits and global Undo/Redo via owned XTEST. Default Glow software host; no Vulkan pixel or full renderer proof."},indent=2)+"\n")
print("PASS BIM toggle + two draws + Undo/Redo")
