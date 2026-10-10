#!/usr/bin/env python3
"""Owned XTEST Position input and positive preview pixels on Qt/GL or Vulkan."""
import ctypes as C,ctypes.util,json,os,select,subprocess,sys,tempfile,time,hashlib
from pathlib import Path
from PySide6.QtGui import QImage
from png_capture import valid_png
from navigation_child_cleanup import terminate_owned
from walk_overlay_pixels import inspect_walk_overlay
os.environ['INGETRAZO_NAV_WALK']='1'
original=os.environ.get('INGETRAZO_POSITION_ORIGINAL')=='1';cancel=os.environ.get('INGETRAZO_POSITION_CANCEL')=='1';typed=os.environ.get('INGETRAZO_POSITION_TYPED_HEIGHT')=='1';look=os.environ.get('INGETRAZO_POSITION_LOOK_DRAG')=='1'
directory=Path(sys.argv[1]).resolve();directory.mkdir(parents=True,exist_ok=True)
assert not (directory/'report.json').exists() and not(directory/'state.json').exists(),'fresh evidence directory required'
binary=(Path(os.environ['CARGO_TARGET_DIR'])/'debug/examples/position_navigation_native_fixture').resolve()
source_revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
def wait(fn,seconds=15):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  try:
   v=fn()
   if v:return v
  except (OSError,ValueError,KeyError):pass
  time.sleep(.03)
 raise AssertionError('Position rendered observation deadline')
def inspect(image,rect,pixels):
 assert not image.isNull() and image.width()>100 and image.height()>100,'missing/readable image'
 x0,y0,x1,y1=map(int,rect);samples=[image.pixelColor(x,y)for x in range(x0+20,x1-20,20)for y in range(y0+20,y1-20,20)]
 assert samples and sum(max(c.red(),c.green(),c.blue())>30 for c in samples)/len(samples)>.9,'black/unpresented world region'
 a,b=pixels;seen=0;count=48
 for i in range(count):
  t=.2+.6*i/(count-1);cx=round(a[0]*(1-t)+b[0]*t);cy=round(a[1]*(1-t)+b[1]*t)
  assert x0<=cx<x1 and y0<=cy<y1,'projected preview outside world region'
  colors=[image.pixelColor(x,y)for x in range(cx-3,cx+4)for y in range(cy-3,cy+4)]
  seen+=any(c.red()>150 and 50<c.green()<180 and c.blue()<110 and c.red()>c.green()+50 for c in colors)
 return dict(samples=count,visible=seen,coverage=seen/count)
server=app=None;d=None
with tempfile.TemporaryDirectory(prefix='position-render-') as runtime:
 try:
  server=subprocess.Popen(['Xvfb','-displayfd','1','-screen','0','1300x1000x24','-nolisten','tcp'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  assert select.select([server.stdout],[],[],10)[0];display=':'+server.stdout.readline().strip();env=dict(os.environ,DISPLAY=display,XDG_RUNTIME_DIR=runtime,XDG_CONFIG_HOME=runtime,LIBGL_ALWAYS_SOFTWARE='1',VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',INGETRAZO_VULKAN_DEVICE='llvmpipe');env.pop('WAYLAND_DISPLAY',None)
  command=[sys.executable,'scripts/original_position_navigation_fixture.py',str(directory)]if original else[str(binary),str(directory)]
  with (directory/'runtime.log').open('w')as log:
   app=subprocess.Popen(command,env=env,stdout=log,stderr=log);state=lambda:json.loads((directory/'state.json').read_text());initial=wait(lambda:state()if state().get('viewport')and(not original or state().get('gl_metadata'))else None)
   if original:assert 'llvmpipe' in initial['gl_metadata']['renderer'].lower(),initial['gl_metadata']
   x=C.CDLL(ctypes.util.find_library('X11'));xt=C.CDLL(ctypes.util.find_library('Xtst'));x.XOpenDisplay.argtypes=[C.c_char_p];x.XOpenDisplay.restype=C.c_void_p;d=x.XOpenDisplay(display.encode());assert d
   x.XDefaultRootWindow.argtypes=[C.c_void_p];x.XDefaultRootWindow.restype=C.c_ulong;root=x.XDefaultRootWindow(d);x.XQueryTree.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ulong)),C.POINTER(C.c_uint)];rr=C.c_ulong();pp=C.c_ulong();children=C.POINTER(C.c_ulong)();n=C.c_uint();assert x.XQueryTree(d,root,C.byref(rr),C.byref(pp),C.byref(children),C.byref(n))
   x.XFetchName.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_char_p)];x.XFree.argtypes=[C.c_void_p];names=[]
   for i in range(n.value):
    name=C.c_char_p()
    if x.XFetchName(d,children[i],C.byref(name))and name.value:names.append((children[i],name.value.decode()));x.XFree(name)
   x.XFree(children);(directory/'window-names.json').write_text(json.dumps(names));owned=[w for w,name in names if name.startswith('IngeTrazo')];assert original or len(owned)==1,'ambiguous owned app window';window=initial['window_id']if original else owned[0]
   x.XSetInputFocus.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_ulong];x.XFlush.argtypes=[C.c_void_p];x.XSetInputFocus(d,window,1,0);x.XFlush(d);wait(lambda:state()['focused'])
   x.XWarpPointer.argtypes=[C.c_void_p,C.c_ulong,C.c_ulong,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_int,C.c_int];xt.XTestFakeButtonEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong];xt.XTestFakeKeyEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong];x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint
   def move(p):x.XWarpPointer(d,0,window,0,0,0,0,*map(round,p));x.XFlush(d);wait(lambda:state()if state().get('pointer')and max(abs(v-w)for v,w in zip(state()['pointer'],map(round,p)))<=1 else None)
   def capture(label):
    dest=directory/(label+'.png');subprocess.run(['import','-display',display,'-window',str(window),'-define','png:color-type=6',str(dest)],check=True,timeout=10);assert valid_png(dest);(directory/(label+'-state.json')).write_text(json.dumps(state(),indent=2)+'\n');return QImage(str(dest))
   rect=initial['viewport'];p=[round((rect[0]+rect[2])/2+90),round((rect[1]+rect[3])/2+45)];end=[p[0],p[1]-60];move(p)
   baseline=capture('before')
   def inspect_walk(image):return inspect_walk_overlay(image,baseline,rect,p)
   assert xt.XTestFakeButtonEvent(d,1,1,0);x.XFlush(d);wait(lambda:state()if state()['walk_anchor']is not None else None);move(end);walking=wait(lambda:state()if state()['eye'][1]>initial['eye'][1]+.01 else None);image=capture('walking');hits=inspect_walk(image)
   assert max(abs(a-b)for a,b in zip(state()['walk_anchor'],p))<=1,'wrong click anchor'
   assert all(abs(walking['eye'][i]-initial['eye'][i])<2e-5 for i in [0,2]),'Walk wrong axis/height'
   negatives=[]
   for label,img in [('missing-cross',baseline),('black-world',QImage(image.size(),QImage.Format_RGBA8888))]:
    if label=='black-world':img.fill(0)
    try:inspect_walk(img)
    except AssertionError:negatives.append(label)
    else:raise AssertionError(label+' admitted')
   if cancel:
    code=x.XKeysymToKeycode(d,0xff1b);assert xt.XTestFakeKeyEvent(d,code,1,0);x.XFlush(d);time.sleep(.03);assert xt.XTestFakeKeyEvent(d,code,0,0);x.XFlush(d)
   assert xt.XTestFakeButtonEvent(d,1,0,0);x.XFlush(d);final=wait(lambda:state()if state()['walk_anchor']is None else None);time.sleep(.15);settled=state();time.sleep(.15);assert state()['eye']==settled['eye'],'Walk continued after retirement';retired=capture('cancelled'if cancel else'released')
   try:inspect_walk(retired)
   except AssertionError:pass
   else:raise AssertionError('retired Walk overlay remained visible')
   assert all(row['undo']==0 and row['version']==initial['version']for row in [walking,final,settled]),'Walk changed model history'
   report=dict(passed=True,source_revision=source_revision,script_sha256=script_sha256,implementation='original-Qt-GL-llvmpipe'if original else'native-Vulkan-llvmpipe',command=command,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest()if not original else None,initial=initial,walking=walking,final=final,settled=settled,overlay_cross_samples=hits,negatives=negatives,cancel=cancel,scope='actual real XTEST press/held-motion/wall-clock walking/release or Escape and positive crosshair; no exact timing/wholeoverlayAA/hardware claim')
   if not original:assert 'Native Vulkan adapter: "llvmpipe' in (directory/'runtime.log').read_text()
   (directory/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(dict(passed=True,cancel=cancel,overlay_cross_samples=hits)))

 except BaseException as error:
  (directory/'failure.json').write_text(json.dumps({'error':repr(error),'state':state()if app is not None and(directory/'state.json').exists()else None},indent=2)+'\n');raise
 finally:
  cleanup={}
  if app is not None:cleanup['app']=terminate_owned(app,5)
  if d:x.XCloseDisplay.argtypes=[C.c_void_p];x.XCloseDisplay(d)
  if server is not None:cleanup['server']=terminate_owned(server,5)
  (directory/'cleanup.json').write_text(json.dumps(cleanup,indent=2)+'\n')
  assert not any(row['kill_timeout']for row in cleanup.values()),'owned cleanup deadline; see cleanup.json'
