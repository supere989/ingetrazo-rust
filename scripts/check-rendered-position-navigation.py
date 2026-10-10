#!/usr/bin/env python3
"""Owned XTEST Position input and positive preview pixels on Qt/GL or Vulkan."""
import ctypes as C,ctypes.util,json,os,select,subprocess,sys,tempfile,time,hashlib
from pathlib import Path
from PySide6.QtGui import QImage
from png_capture import valid_png
original=os.environ.get('INGETRAZO_POSITION_ORIGINAL')=='1';cancel=os.environ.get('INGETRAZO_POSITION_CANCEL')=='1';typed=os.environ.get('INGETRAZO_POSITION_TYPED_HEIGHT')=='1'
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
   rect=initial['viewport'];p=[(rect[0]+rect[2])/2+90,(rect[1]+rect[3])/2+45];end=[p[0]+65,p[1]-40];move(p)
   typed_state=None
   if typed:
    wait(lambda:state()if state().get('measurement')=='1.68 m'else None)
    if not original:
     box=state()['measurement_rect'];move([(box[0]+box[2])/2,(box[1]+box[3])/2]);assert xt.XTestFakeButtonEvent(d,1,1,0);x.XFlush(d);time.sleep(.05);assert xt.XTestFakeButtonEvent(d,1,0,0);x.XFlush(d);time.sleep(.08)
    def key(symbol):
     code=x.XKeysymToKeycode(d,symbol);assert xt.XTestFakeKeyEvent(d,code,1,0);x.XFlush(d);time.sleep(.03);assert xt.XTestFakeKeyEvent(d,code,0,0);x.XFlush(d);time.sleep(.03)
    if not original:
     control=x.XKeysymToKeycode(d,0xffe3);assert xt.XTestFakeKeyEvent(d,control,1,0);x.XFlush(d);time.sleep(.03);key(ord('a'));assert xt.XTestFakeKeyEvent(d,control,0,0);x.XFlush(d);time.sleep(.03)
    for char in '2.5':key(ord(char))
    key(0xff0d)
    if original:
     accepted=wait(lambda:state()if state().get('eye_height')==2.5 else None);move([p[0]+1,p[1]])
    typed_state=wait(lambda:state()if state().get('measurement')=='2.50 m'else None);assert typed_state['eye']==initial['eye']and typed_state['undo']==0,'typed height before placement mutated camera/history';move(p)
   baseline=capture('before')
   endpoint=state()['endpoint_pixel'];move(endpoint);glyph_state=wait(lambda:state()if str(state().get('snap_kind')).lower()=='endpoint'else None);time.sleep(.08);glyph=capture('endpoint-glyph')
   def green(image):
    cx,cy=map(round,endpoint);colors=[image.pixelColor(x,y)for x in range(cx-8,cx+9)for y in range(cy-8,cy+9)];return sum(c.green()>80 and c.green()>c.red()+30 and c.green()>c.blue()+20 for c in colors)
   glyph_green=green(glyph);assert glyph_green>5,('missing Position endpoint glyph',glyph_green);assert green(baseline)==0,'baseline green negative';move(p)
   assert xt.XTestFakeButtonEvent(d,1,1,0);x.XFlush(d);time.sleep(.05);move(end);preview=wait(lambda:state()if state()['preview']is not None else None);time.sleep(.08);image=capture('preview');pixels=preview['preview_pixels'];coverage=inspect(image,rect,pixels);assert coverage['coverage']>=.95,coverage
   assert preview['undo']==0 and preview['version']==initial['version']and preview['eye']==initial['eye'],'premature geometry/camera mutation'
   negative=[]
   black=QImage(image.size(),QImage.Format_RGBA8888);black.fill(0)
   try:inspect(black,rect,pixels)
   except AssertionError:negative.append('black-world')
   else:raise AssertionError('black-world negative admitted')
   assert inspect(baseline,rect,pixels)['coverage']<.2,'before image unexpectedly contains preview';negative.append('missing-preview')
   if cancel:
    code=x.XKeysymToKeycode(d,0xff1b);assert xt.XTestFakeKeyEvent(d,code,1,0);x.XFlush(d);time.sleep(.05);assert xt.XTestFakeKeyEvent(d,code,0,0);x.XFlush(d);wait(lambda:state()if state()['preview']is None else None)
   assert xt.XTestFakeButtonEvent(d,1,0,0);x.XFlush(d)
   final=wait(lambda:state()if state()['preview']is None and(cancel or 'Look' in state()['mode'])else None);time.sleep(.08);capture('cancelled'if cancel else'committed')
   assert final['undo']==0 and final['version']==initial['version'];assert (final['eye']==initial['eye'])==cancel
   if typed and not cancel:assert abs(final['eye'][2]-2.5)<2e-5 and final['measurement']=='2.50 m',final
   if not original and not cancel:assert final['status'].startswith('Look Around'),final
   report=dict(passed=True,source_revision=source_revision,script_sha256=script_sha256,implementation='original-Qt-OpenGL-llvmpipe'if original else'native-Vulkan-llvmpipe',cancel=cancel,typed_height=typed,typed_state=typed_state,coverage=coverage,glyph_green=glyph_green,glyph_state=glyph_state,negatives=negative+['missing-endpoint-glyph'],initial=initial,preview=preview,final=final,command=command,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest()if not original else None,scope='actual XTEST pointer/Escape and visible projected preview; no whole-image glyph/AA parity or hardware claim')
   if not original:assert 'Native Vulkan adapter: \"llvmpipe' in (directory/'runtime.log').read_text(),'actual Vulkan adapter not observed'
   (directory/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'passed':True,'coverage':coverage,'cancel':cancel}))
 except BaseException as error:
  (directory/'failure.json').write_text(json.dumps({'error':repr(error),'state':state()if app is not None and(directory/'state.json').exists()else None},indent=2)+'\n');raise
 finally:
  if app is not None:
   app.terminate()
   try:app.wait(timeout=5)
   except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
  if d:x.XCloseDisplay.argtypes=[C.c_void_p];x.XCloseDisplay(d)
  if server is not None:
   server.terminate()
   try:server.wait(timeout=5)
   except subprocess.TimeoutExpired:server.kill();server.wait(timeout=5)
