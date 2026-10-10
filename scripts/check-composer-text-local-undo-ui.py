#!/usr/bin/env python3
"""Actual original/native Composer Text authoring through XTEST; no editor pixel parity claim."""
import os,json,subprocess,tempfile,time,select,ctypes as C,ctypes.util,hashlib,math,sys
from pathlib import Path
from types import SimpleNamespace
from PySide6.QtWidgets import QApplication,QGraphicsScene
from PySide6.QtGui import QImage,QPainter,QPen,QBrush,QColor
from PySide6.QtCore import Qt,QRectF
appqt=QApplication([]);assert __import__('PySide6.QtCore',fromlist=['qVersion']).qVersion()=='6.12.0';root=Path(__file__).resolve().parents[1];sys.path.insert(0,'/work/ingetrazo');from views.composer import paint_sheet_border_mm,TextItem
from core.composition import TextoItem
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd='/work/ingetrazo',text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
out=Path(os.environ.get('INGETRAZO_BORDER_EVIDENCE',str(root/'docs/audits/composer-text-authoring/native-ui')));out.mkdir(parents=True,exist_ok=True);(out/'state.json').unlink(missing_ok=True);(out/'report.json').unlink(missing_ok=True)
binary=Path(os.environ['CARGO_TARGET_DIR'])/'debug/examples/composer_text_authoring_native_fixture'
def wait(f):
 end=time.monotonic()+20
 while time.monotonic()<end:
  try:
   v=f()
   if v:return v
  except (OSError,ValueError):pass
  time.sleep(.05)
 raise AssertionError('Owned Text authoring observation timeout')
results=[]
with tempfile.TemporaryDirectory()as runtime:
 server=subprocess.Popen(['Xvfb','-displayfd','1','-screen','0','1600x1000x24','-nolisten','tcp'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);process=None
 try:
  assert select.select([server.stdout],[],[],10)[0];display=':'+server.stdout.readline().strip();env=dict(os.environ,DISPLAY=display,XDG_RUNTIME_DIR=runtime,VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',INGETRAZO_VULKAN_DEVICE='llvmpipe',INGETRAZO_PDF_UI_OBSERVE='1',INGETRAZO_TEXT_AUTHORING_OUT=str(out));env.pop('WAYLAND_DISPLAY',None)
  with(out/'runtime.log').open('w')as log:
   env['QT_QPA_PLATFORM']='xcb';process=subprocess.Popen(([sys.executable,str(root/'scripts/composer-text-authoring-original-ui.py')] if os.environ.get('INGETRAZO_TEXT_ORIGINAL_UI') else [str(binary)]),env=env,stdout=log,stderr=log)
   x=C.CDLL(ctypes.util.find_library('X11'));x.XOpenDisplay.argtypes=[C.c_char_p];x.XOpenDisplay.restype=C.c_void_p;d=x.XOpenDisplay(display.encode());assert d;x.XDefaultRootWindow.argtypes=[C.c_void_p];x.XDefaultRootWindow.restype=C.c_ulong;windowroot=x.XDefaultRootWindow(d)
   x.XQueryTree.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ulong)),C.POINTER(C.c_uint)];rr,pp,nn=C.c_ulong(),C.c_ulong(),C.c_uint();children=C.POINTER(C.c_ulong)()
   def window():
    x.XQueryTree(d,windowroot,C.byref(rr),C.byref(pp),C.byref(children),C.byref(nn));return children[0]if nn.value else None
   wid=wait(lambda:json.loads((out/"state.json").read_text())["window_id"])if os.environ.get("INGETRAZO_TEXT_ORIGINAL_UI")else wait(window);x.XSetInputFocus.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_ulong];x.XFlush.argtypes=[C.c_void_p];x.XSetInputFocus(d,wid,1,0);x.XFlush(d);x.XWarpPointer.argtypes=[C.c_void_p,C.c_ulong,C.c_ulong,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_int,C.c_int];x.XFlush.argtypes=[C.c_void_p];xt=C.CDLL(ctypes.util.find_library('Xtst'));xt.XTestFakeButtonEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
   def click(point):
    x.XWarpPointer(d,0,wid,0,0,0,0,round(point[0]),round(point[1]));x.XFlush(d);time.sleep(.03)
    for down in (1,0):assert xt.XTestFakeButtonEvent(d,1,down,0);x.XFlush(d);time.sleep(.03)
   state=lambda:json.loads((out/'state.json').read_text())
   x.XStringToKeysym.argtypes=[C.c_char_p];x.XStringToKeysym.restype=C.c_ulong;x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint;xt.XTestFakeKeyEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
   def key(name,ctrl=False,shift=False):
    def event(n,down):
     n='space' if n==' ' else n;code=x.XKeysymToKeycode(d,x.XStringToKeysym(n.encode()));assert code>0,(n,code);assert xt.XTestFakeKeyEvent(d,code,down,0);x.XFlush(d)
    if ctrl:event('Control_L',1)
    if shift:event('Shift_L',1)
    event(name,1);event(name,0)
    if shift:event('Shift_L',0)
    if ctrl:event('Control_L',0)
   def record(label):
    value=state();results.append(dict(label=label,state=value));subprocess.run(['import','-display',display,'-window',str(wid),str(out/(label+'.png'))],check=True);return value
   initial=wait(lambda:state()if state().get('add_text')and state().get('paper_transform')else None);click(initial['add_text']);wait(lambda:state()['text_tool']);origin=state()['paper_transform'];point=[origin[0]+70*origin[2],origin[1]+80*origin[2]];click(point);wait(lambda:len(state()['texts'])==1 and not state()['text_tool']);placed=record('placed');assert placed['history']==[1,0]
   click([point[0]+200,point[1]+100]);wait(lambda:state()['selected'] is None);click(point);click(point);wait(lambda:state()['inline'] is not None);key('a',True)
   for c in 'hello':key(c)
   key('Return')
   for c in 'world':key(c)
   wait(lambda:state()['inline']=='hello\nworld');draft=record('draft');assert draft['texts'][0]['text']=='Text'and draft['history']==[1,0]
   def expect(label,text):wait(lambda:state()['inline']==text);v=record(label);assert v['history']==[1,0] and v['texts'][0]['text']=='Text'
   for label,text in [('undo-world','hello\n'),('undo-newline','hello'),('undo-replacement','Text')]:key('z',True);expect(label,text)
   for label,text in [('redo-replacement','hello'),('redo-newline','hello\n'),('redo-world','hello\nworld')]:key('z',True,True);expect(label,text)
   key('a',True)
   for c in 'alpha beta':key(c)
   expect('held-replacement','alpha beta');key('z',True);expect('held-replacement-undo','hello\nworld');key('z',True,True);expect('held-replacement-redo','alpha beta')
   key('Left');key('Left');key('x');expect('held-cursor-insert','alpha bexta');key('z',True);expect('held-cursor-undo','alpha beta');key('z',True,True);expect('held-cursor-redo','alpha bexta')
   key('BackSpace');key('BackSpace');expect('held-backspace','alpha bta');key('z',True);expect('held-backspace-undo','alpha beta');key('z',True,True);expect('held-backspace-redo','alpha bta')
   key('Return',True);wait(lambda:state()['inline'] is None and state()['texts'][0]['text']=='alpha bta');committed=record('held-commit');assert committed['history']==[2,0]
   if os.environ.get('INGETRAZO_TEXT_ORIGINAL_UI'):process.terminate();process.wait(timeout=10)
   else:click(state()['close']);assert process.wait(timeout=10)==0;assert 'committed_bytes=0'in(out/'runtime.log').read_text()
 finally:
  if process and process.poll()is None:process.terminate();process.wait(timeout=10)
  server.terminate();server.wait(timeout=10)
(out/'report.json').write_text(json.dumps({'passed':True,'binary_sha256':hashlib.sha256(Path(sys.executable).read_bytes() if os.environ.get('INGETRAZO_TEXT_ORIGINAL_UI') else binary.read_bytes()).hexdigest(),'original_fixture_sha256':hashlib.sha256((root/'scripts/composer-text-authoring-original-ui.py').read_bytes()).hexdigest() if os.environ.get('INGETRAZO_TEXT_ORIGINAL_UI') else None,'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','scope':('Actual original MainWindow/ComposerWindow and unmodified source widgets, XTEST input; source Escape caller shadowing retained' if os.environ.get('INGETRAZO_TEXT_ORIGINAL_UI') else 'Actual normal Composer button/pointer/double-click/keyboard Text placement/edit/CtrlEnter/UndoRedo/Escape/outside through native Vulkan public update_ui; editor typography not pixel-compared'),'states':results},indent=2)+'\n')
print('PASS actual source/native held-out local document Undo/Redo gestures')
