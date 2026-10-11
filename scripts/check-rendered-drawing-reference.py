#!/usr/bin/env python3
"""Actual native pointer/key events through a rendered production viewport."""
import argparse,ctypes as C,ctypes.util,json,os,subprocess,tempfile,time,select,struct,zlib,copy
from pathlib import Path
from acceptance_context import context
from png_capture import valid_png
def wait(fn,seconds=20):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  try:
   v=fn()
   if v:return v
  except (OSError,ValueError):pass
  time.sleep(.05)
 raise AssertionError("Timed out awaiting rendered event/state")
def assert_cancelled_rectangle(observed,initial):
 assert observed["pending_anchor"] is None and observed["drawing_plane"] is None,"Rectangle acquisition/plane remains armed"
 assert all(observed[k]==initial[k] for k in ("faces","edges","undo","vertices")),"Escape changed geometry/history"
 # Literal source reset keeps hover and therefore the four-edge idle cursor.
 cursor=observed["preview"];assert len(cursor)==4,"Expected source idle Rectangle cursor"
 assert all(line[1]==cursor[(i+1)%4][0] for i,line in enumerate(cursor)),"Cursor loop is not closed"

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument("--prebuilt-dir",type=Path,help="Explicit freshly built native example directory; no Cargo invocation")
 args=parser.parse_args()
 if args.prebuilt_dir is None:
  subprocess.run(["cargo","build","--locked","-j2","-p","ingetrazo-gui","--example","drawing_reference_render_fixture"],check=True)
  binary=Path(os.environ.get("CARGO_TARGET_DIR","target"))/"debug/examples/drawing_reference_render_fixture"
 else:binary=args.prebuilt_dir/"drawing_reference_render_fixture"
 assert binary.is_file(),"Missing explicitly selected example binary"
 binary=binary.resolve()
 directory=Path(os.environ.get("INGETRAZO_RENDER_AUDIT_DIR","docs/audits/drawing-reference-render"));directory.mkdir(exist_ok=True)
 for p in directory.iterdir():
  if p.name in ("state.json","capture.txt","before.png","preview.png","cancelled.png","committed.png","committed.igz"):p.unlink()
 with tempfile.TemporaryDirectory(prefix="ingetrazo-reference-x11-") as t:
  server=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1200x900x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  app=None
  try:
   assert select.select([server.stdout],[],[],10)[0];display=server.stdout.readline().strip();assert display.isdigit()
   env=os.environ.copy();env["DISPLAY"]=":"+display;env["XDG_RUNTIME_DIR"]=t;env["LIBGL_ALWAYS_SOFTWARE"]="1";env.pop("WAYLAND_DISPLAY",None)
   log=open(directory/"runtime.log","w");app=subprocess.Popen([str(binary),str(directory.resolve())],env=env,stdout=log,stderr=log)
   state=lambda:json.loads((directory/"state.json").read_text())
   initial=wait(lambda:state() if state()["faces"]==0 else None)
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
    (directory/"capture.txt").write_text(name);wait(lambda:valid_png(directory/(name+".png")))
   x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint
   def key(symbol):
    code=x.XKeysymToKeycode(d,symbol);event(2,code,None,1);event(3,code,None,2)
   capture("before");move(initial["click"]);key(0xff54);click(initial["click"]);move(initial["move"])
   preview=wait(lambda:state() if state()["pending_anchor"] is not None and len(state()["preview"])>=4 else None);assert preview["faces"]==0 and preview["edges"]==1 and preview["undo"]==0
   origin,normal=initial["origin"],initial["normal"]
   dot=lambda a,b:sum(x*y for x,y in zip(a,b))
   error=max(abs(dot([p[i]-origin[i]for i in range(3)],normal))for line in preview["preview"]for p in line);assert error<1e-6,error
   capture("preview");key(0xff1b)
   cancelled=wait(lambda:state() if state()["pending_anchor"] is None and state()["drawing_plane"] is None else None)
   assert_cancelled_rectangle(cancelled,initial)
   negatives=[]
   for name,mutate in [("unfinished-acquisition",lambda x:x.update(pending_anchor=preview["pending_anchor"])),("unfinished-plane",lambda x:x.update(drawing_plane=preview["drawing_plane"])),("changed-mesh",lambda x:x["vertices"][0].__setitem__(0,x["vertices"][0][0]+1)),("changed-history",lambda x:x.update(undo=x["undo"]+1)),("missing-idle-cursor",lambda x:x.update(preview=[]))]:
    wrong=copy.deepcopy(cancelled);mutate(wrong)
    try:assert_cancelled_rectangle(wrong,initial)
    except AssertionError:negatives.append(name)
    else:raise AssertionError("Cancelled Rectangle corruption accepted: "+name)
   capture("cancelled")
   move(initial["click"]);key(0xff54);click(initial["click"]);move(initial["move"]);wait(lambda:state()["pending_anchor"] is not None and len(state()["preview"])>=4);click(initial["move"])
   committed=wait(lambda:state() if state()["faces"]==1 else None);assert committed["pending_anchor"] is None and committed["drawing_plane"] is None;assert committed["undo"]==1;assert abs(committed["face_areas"][0]-1.8)<.06,committed
   alignment=abs(dot(committed["face_normals"][0],normal));assert alignment>0.999999,alignment
   capture("committed")
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
   changed=sum(a!=b for a,b in zip(before,preview_image));assert changed>100,"preview must materially render"
   committed_image=pixels(directory/"committed.png");changed_commit=sum(a!=b for a,b in zip(before,committed_image));assert changed_commit>100
   report={"provenance":context(),"passed":True,"native_events":"XTEST pointer/button/Down/Escape events on isolated Xvfb","preview_changed_pixels":changed,"committed_changed_pixels":changed_commit,"preview_plane_error":error,"committed_normal_alignment":alignment,"initial":initial,"preview":preview,"cancelled":cancelled,"cancellation_negatives":negatives,"committed":committed,"scope":"Actual production render_viewport Rectangle Down-before-first-click on nonprincipal edge; pixel midpoint snapping; moving coplanar preview; Escape leaves mesh/undo unchanged; second click authors one face and one undo. Bounded engineering proof using existing software-GL test backend, not Vulkan/native GPU parity or full app menu/keyboard proof."}
   Path(os.environ.get("INGETRAZO_RENDER_AUDIT_REPORT","docs/audits/drawing-reference-render-evidence.json")).write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
  finally:
   if app:app.terminate();app.wait(timeout=5)
   server.terminate();server.communicate(timeout=5)
if __name__=="__main__":main()
