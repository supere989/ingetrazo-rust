#!/usr/bin/env python3
"""Actual native pointer/key events through a rendered production viewport."""
import ctypes as C,ctypes.util,json,os,subprocess,tempfile,time,select,struct,zlib
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
 subprocess.run(["cargo","build","--locked","-p","ingetrazo-gui","--example","offset_render_acceptance_fixture"],check=True)
 directory=Path(os.environ.get("INGETRAZO_RENDER_AUDIT_DIR","docs/audits/offset-render"));directory.mkdir(exist_ok=True)
 for p in directory.iterdir():
  if p.name in ("state.json","capture.txt","before.png","preview.png","cancelled.png","committed.png","committed.igz"):p.unlink()
 with tempfile.TemporaryDirectory(prefix="ingetrazo-offset-x11-") as t:
  server=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1200x900x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  app=None
  try:
   assert select.select([server.stdout],[],[],10)[0];display=server.stdout.readline().strip();assert display.isdigit()
   env=os.environ.copy();env["DISPLAY"]=":"+display;env["XDG_RUNTIME_DIR"]=t;env["LIBGL_ALWAYS_SOFTWARE"]="1";env.pop("WAYLAND_DISPLAY",None)
   log=open(directory/"runtime.log","w");app=subprocess.Popen(["target/debug/examples/offset_render_acceptance_fixture",str(directory.resolve())],env=env,stdout=log,stderr=log)
   state=lambda:json.loads((directory/"state.json").read_text())
   initial=wait(lambda:state() if state()["faces"]==1 else None)
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
   capture("before")
   click(initial["click"]);move(initial["move"])
   preview=wait(lambda:state() if len(state()["preview"])==4 else None);assert preview["faces"]==1 and preview["undo"]==0
   coords=[p for segment in preview["preview"] for p in segment];assert abs(min(p[0] for p in coords)-.5)<.03 and abs(max(p[0] for p in coords)-3.5)<.03
   capture("preview")
   x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint;key=x.XKeysymToKeycode(d,0xff1b);event(2,key,initial["move"],1);event(3,key,initial["move"],2)
   wait(lambda:not state()["preview"]);assert state()["faces"]==1 and state()["undo"]==0;capture("cancelled")
   click(initial["click"]);move(initial["move"]);wait(lambda:len(state()["preview"])==4);click(initial["move"])
   committed=wait(lambda:state() if state()["faces"]==2 else None);assert committed["undo"]==1;capture("committed")
   def pixels(path):
    data=path.read_bytes();assert data[:8]==b"\x89PNG\r\n\x1a\n";at=8;packed=b""
    while at<len(data):
     n=struct.unpack_from(">I",data,at)[0];tag=data[at+4:at+8];body=data[at+8:at+8+n];at+=12+n
     if tag==b"IHDR":width,height,depth,color=struct.unpack_from(">IIBB",body);assert depth==8 and color==6
     if tag==b"IDAT":packed+=body
    raw=zlib.decompress(packed);stride=width*4;out=[];previous=[0]*stride;at=0
    for _ in range(height):
     mode=raw[at];row=list(raw[at+1:at+1+stride]);at+=stride+1
     for j in range(stride):
      a=row[j-4] if j>=4 else 0;b=previous[j];c=previous[j-4] if j>=4 else 0
      if mode==1:predict=a
      elif mode==2:predict=b
      elif mode==3:predict=(a+b)//2
      elif mode==4:
       p=a+b-c;pa,pb,pc=abs(p-a),abs(p-b),abs(p-c);predict=a if pa<=pb and pa<=pc else b if pb<=pc else c
      else:assert mode==0;predict=0
      row[j]=(row[j]+predict)%256
     out.extend(tuple(row[j:j+4]) for j in range(0,stride,4));previous=row
    return out
   before,preview_image=pixels(directory/"before.png"),pixels(directory/"preview.png")
   changed=sum(a!=b for a,b in zip(before,preview_image));assert changed>1000,"preview must materially render"
   cancelled=pixels(directory/"cancelled.png")
   cancel_changed=sum(a!=b for a,b in zip(before,cancelled));assert cancel_changed<500,"cancel must remove rendered inset"
   red=lambda p:p[0]>240 and 40<p[1]<90 and 40<p[2]<90
   added_red=sum(red(b) and not red(a) for a,b in zip(before,preview_image));assert added_red>1000,"preview inset must add red geometry pixels"
   committed_image=pixels(directory/"committed.png")
   dark=lambda p:5<p[0]<180 and 5<p[1]<180 and 5<p[2]<180
   committed_dark=sum(dark(b) and not dark(a) for a,b in zip(before,committed_image));assert committed_dark>1000,"committed hard inset must remain visibly rendered"
   report={"provenance":context(),"passed":True,"native_events":"X11 pointer warp and XTEST native button/escape key synthesis on disposable Xvfb","preview_changed_pixels":changed,"preview_added_red_pixels":added_red,"cancel_changed_pixels":cancel_changed,"committed_added_dark_pixels":committed_dark,"initial":initial,"preview":preview,"committed":committed,"scope":"Production render_viewport within native eframe harness: face pointer pick, perpendicular moving preview rendered, Escape cancel no model/history mutation, second-click commit one undo and visible hard inner boundary. This isolates viewport gestures; full app menus/tool selection/VCB/undo keyboard routing and other display platforms remain open."}
   Path(os.environ.get("INGETRAZO_RENDER_AUDIT_REPORT","docs/audits/offset-render-semantic-evidence.json")).write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
  finally:
   if app:app.terminate();app.wait(timeout=5)
   server.terminate();server.communicate(timeout=5)
if __name__=="__main__":main()
