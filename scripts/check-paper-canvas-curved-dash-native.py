#!/usr/bin/env python3
"""Owned Vulkan/XTEST border changes + actual Qt border pixels + warm upload counters."""
import os,json,subprocess,tempfile,time,select,ctypes as C,ctypes.util,hashlib,math,sys
from pathlib import Path
from types import SimpleNamespace
from PySide6.QtWidgets import QApplication,QGraphicsScene
from PySide6.QtGui import QImage,QPainter,QPen,QBrush,QColor
from PySide6.QtCore import Qt,QRectF
appqt=QApplication([]);assert __import__('PySide6.QtCore',fromlist=['qVersion']).qVersion()=='6.12.0';root=Path(__file__).resolve().parents[1];sys.path.insert(0,'/work/ingetrazo');from views.composer import paint_sheet_border_mm
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd='/work/ingetrazo',text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
out=Path(os.environ.get('INGETRAZO_BORDER_EVIDENCE',str(root/'docs/audits/paper-canvas-curved-dash/native')));out.mkdir(exist_ok=True);(out/'state.json').unlink(missing_ok=True)
import textwrap
original=Path('/work/ingetrazo/views/composer.py').read_text();start=original.index('        shadow = self.canvas.addRect(2.0, 2.0, pw, ph, QPen(Qt.NoPen),');end=original.index('        margin.setZValue(-100001)',start)+len('        margin.setZValue(-100001)');source_stack=textwrap.dedent(original[start:end])
cases=[{'name':n}for n in ['production-curved-dash','production-clamped-dash','production-thin-curved-dash','production-carry-dash']]
if os.environ.get('INGETRAZO_COMPOUND_FIRST_ONLY')=='1':cases=cases[:1]
recipes=root/'docs/audits/paper-canvas-border/recipes.json'
binary=Path(os.environ['CARGO_TARGET_DIR'])/'debug/examples/paper_curved_border_native_fixture'
def wait(f):
 end=time.monotonic()+20
 while time.monotonic()<end:
  try:
   v=f()
   if v:return v
  except (OSError,ValueError):pass
  time.sleep(.05)
 raise AssertionError('Owned native border observation timeout')
results=[]
with tempfile.TemporaryDirectory()as runtime:
 server=subprocess.Popen(['Xvfb','-displayfd','1','-screen','0','1600x1000x24','-nolisten','tcp'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);process=None
 try:
  assert select.select([server.stdout],[],[],10)[0];display=':'+server.stdout.readline().strip();env=dict(os.environ,DISPLAY=display,XDG_RUNTIME_DIR=runtime,VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',INGETRAZO_VULKAN_DEVICE='llvmpipe',INGETRAZO_PDF_UI_OBSERVE='1',INGETRAZO_BORDER_OUT=str(out));env.pop('WAYLAND_DISPLAY',None)
  with(out/'runtime.log').open('w')as log:
   process=subprocess.Popen([str(binary)],env=env,stdout=log,stderr=log)
   x=C.CDLL(ctypes.util.find_library('X11'));x.XOpenDisplay.argtypes=[C.c_char_p];x.XOpenDisplay.restype=C.c_void_p;d=x.XOpenDisplay(display.encode());assert d;x.XDefaultRootWindow.argtypes=[C.c_void_p];x.XDefaultRootWindow.restype=C.c_ulong;windowroot=x.XDefaultRootWindow(d)
   x.XQueryTree.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ulong)),C.POINTER(C.c_uint)];rr,pp,nn=C.c_ulong(),C.c_ulong(),C.c_uint();children=C.POINTER(C.c_ulong)()
   def window():
    x.XQueryTree(d,windowroot,C.byref(rr),C.byref(pp),C.byref(children),C.byref(nn));return children[0]if nn.value else None
   wid=wait(window);x.XWarpPointer.argtypes=[C.c_void_p,C.c_ulong,C.c_ulong,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_int,C.c_int];x.XFlush.argtypes=[C.c_void_p];xt=C.CDLL(ctypes.util.find_library('Xtst'));xt.XTestFakeButtonEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
   def click(point):
    x.XWarpPointer(d,0,wid,0,0,0,0,round(point[0]),round(point[1]));x.XFlush(d);time.sleep(.1)
    for down in (1,0):assert xt.XTestFakeButtonEvent(d,1,down,0);x.XFlush(d);time.sleep(.15)
   state=lambda:json.loads((out/'state.json').read_text())
   for i,case in enumerate(cases):
    s=wait(lambda:state()if state()['index']==i and state()['gpu'].get('uploads',0)>0 else None);serial=s['serial'];wait(lambda:state()['serial']>=serial+3);s=state();filename=out/(case['name']+'.png');subprocess.run(['import','-display',display,'-window',str(wid),str(filename)],check=True)
    a=QImage(str(filename));assert not a.isNull();b=QImage(a.size(),QImage.Format_ARGB32_Premultiplied);b.fill(QColor(32,34,40));p=QPainter(b);p.setRenderHints(QPainter.Antialiasing|QPainter.SmoothPixmapTransform);p.translate(*s['origin']);p.scale(s['zoom'],s['zoom']);border=s['border'];model=SimpleNamespace(border=True,page_size_mm=lambda:s['page'],margin_mm=s['margin'],border_mm=border['mm'],border_radius_mm=border['radius_mm'],border_color=border['color'],border_style=border['style']);scene=QGraphicsScene();owner=SimpleNamespace(canvas=scene,comp=model,_add_guide_items=lambda:None);exec(source_stack,globals(),dict(self=owner,pw=s['page'][0],ph=s['page'][1]));view=QRectF(-s['origin'][0]/s['zoom'],-s['origin'][1]/s['zoom'],a.width()/s['zoom'],a.height()/s['zoom']);scene.render(p,view,view,Qt.IgnoreAspectRatio);paint_sheet_border_mm(p,model);p.end();b.save(str(out/(case['name']+'-source.png')))
    z=s['zoom'];roi=[math.floor(s['origin'][0]-1),math.floor(s['origin'][1]-1),math.ceil(s['origin'][0]+(s['page'][0]+2)*z+1),math.ceil(s['origin'][1]+(s['page'][1]+2)*z+1)];assert 0<=roi[0]<roi[2]<=a.width()and 0<=roi[1]<roi[3]<=a.height()
    diff=[(xx,yy)for yy in range(roi[1],roi[3])for xx in range(roi[0],roi[2])if a.pixel(xx,yy)!=b.pixel(xx,yy)]
    warm=[]
    for _ in range(3):before=state();wait(lambda:state()['serial']>=before['serial']+2);warm.append(state())
    assert all(v['cold_builds']==s['cold_builds']and v['gpu']['uploads']==s['gpu']['uploads']and v['gpu']['cached_bytes']==s['gpu']['cached_bytes']for v in warm),'Unchanged border rebuilt/uploaded'
    results.append(dict(name=case['name'],changed=len(diff),first=diff[:5],roi=roi,state=s,warm=warm));
    if i+1<len(cases):click(s['next'])
   click(state()['close']);assert process.wait(timeout=10)==0;assert 'committed_bytes=0'in(out/'runtime.log').read_text()
 finally:
  if process and process.poll()is None:process.terminate();process.wait(timeout=10)
  server.terminate();server.wait(timeout=10)
(out/'report.json').write_text(json.dumps(dict(binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),source_stack_sha256=hashlib.sha256(source_stack.encode()).hexdigest(),scope='Actual production render_sheet_composer page/shadow/margin/border with empty authored items, actual XTEST recipe changes; original operative QGraphicsScene stack plus actual border painter',cases=results),indent=2)+'\n');print([(r['name'],r['changed'])for r in results]);assert all(r['changed']==0 for r in results),'Actual border pixel discrepancy'
