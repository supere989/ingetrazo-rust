#!/usr/bin/env python3
"""Real original composer checkbox and genuine QColorDialog pointer controls."""
import os,json,subprocess,tempfile,time,select,ctypes as C,ctypes.util,signal,hashlib
from pathlib import Path
out=Path(os.environ['INGETRAZO_ORIGINAL_FIELD_UI_EVIDENCE']);out.mkdir(parents=True,exist_ok=True)
def wait(fn):
 end=time.monotonic()+20
 while time.monotonic()<end:
  try:
   result=fn()
   if result:return result
  except (OSError,ValueError):pass
  time.sleep(.05)
 raise AssertionError('Actual source UI timeout')
with tempfile.TemporaryDirectory()as runtime:
 server=subprocess.Popen(['Xvfb','-displayfd','1','-screen','0','1600x1000x24','-nolisten','tcp'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);app=None
 try:
  assert select.select([server.stdout],[],[],10)[0];display=':'+server.stdout.readline().strip()
  env=os.environ.copy();env.update(DISPLAY=display,QT_QPA_PLATFORM='xcb',XDG_RUNTIME_DIR=runtime,INGETRAZO_ORIGINAL_FIELD_UI_OUT=str(out.resolve()));env.pop('WAYLAND_DISPLAY',None)
  log=open(out/'runtime.log','w');app=subprocess.Popen([sys_executable:='/tmp/ingetrazo-parity-python/bin/python','scripts/original_field_border_ui_fixture.py'],env=env,stdout=log,stderr=log,start_new_session=True)
  state=lambda:json.loads((out/'state.json').read_text());before=wait(state)
  x=C.CDLL(ctypes.util.find_library('X11'));x.XOpenDisplay.argtypes=[C.c_char_p];x.XOpenDisplay.restype=C.c_void_p;d=x.XOpenDisplay(display.encode());assert d
  x.XDefaultRootWindow.argtypes=[C.c_void_p];x.XDefaultRootWindow.restype=C.c_ulong;root=x.XDefaultRootWindow(d)
  x.XWarpPointer.argtypes=[C.c_void_p,C.c_ulong,C.c_ulong,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_int,C.c_int];x.XFlush.argtypes=[C.c_void_p];x.XSetInputFocus.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_ulong]
  xt=C.CDLL(ctypes.util.find_library('Xtst'));xt.XTestFakeButtonEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong];xt.XTestFakeKeyEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
  x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint
  def click(point):
   x.XWarpPointer(d,0,root,0,0,0,0,round(point[0]),round(point[1]));x.XFlush(d);time.sleep(.1)
   for down in (1,0):assert xt.XTestFakeButtonEvent(d,1,down,0);x.XFlush(d);time.sleep(.15)
  def key(symbol,modifier=None):
   if modifier:xt.XTestFakeKeyEvent(d,x.XKeysymToKeycode(d,modifier),1,0)
   code=x.XKeysymToKeycode(d,symbol);assert code;xt.XTestFakeKeyEvent(d,code,1,0);xt.XTestFakeKeyEvent(d,code,0,0)
   if modifier:xt.XTestFakeKeyEvent(d,x.XKeysymToKeycode(d,modifier),0,0)
   x.XFlush(d);time.sleep(.02)
  def capture(name,wid=None):subprocess.run(['import','-display',display,'-window',str(wid or before['window_id']),str(out/(name+'.png'))],check=True)
  x.XSetInputFocus(d,before['window_id'],1,0);capture('before');assert before['source_runtime_equal_second']
  click(before['toggle']);wait(lambda:state()['border']['on']is True)
  click(before['color_button']);dialog=wait(lambda:state()if state()['dialog_hexes']and state()['dialog_ok']else None);capture('genuine-color-dialog',dialog['dialog_id'])
  field=next(item for item in dialog['dialog_hexes']if item['name']=='qt_colorname' or item['text'].startswith('#'))
  click(field['point']);key(ord('a'),0xffe3)
  for char in '#ffffff':key(ord(char),0xffe1 if char=='#'else None)
  capture('typed-color-dialog',dialog['dialog_id']);click(dialog['dialog_ok']);wait(lambda:state()['border']['color']=='#ffffff')
  capture('white-enabled');click(before['toggle']);after=wait(lambda:state()if state()['border']['on']is False else None);capture('white-disabled')
  assert after['border']['color']=='#ffffff' and not after['source_runtime_equal_second'] and after['authored_text']==before['authored_text']
  key(ord('z'),0xffe3);after_undo=wait(lambda:state()if state()['source_undo_activations']==1 else None);capture('source-property-undo')
  assert after_undo['border']==after['border'] and after_undo['undo']==after_undo['redo']==0
  report={'passed':True,'before':before,'after':after,'after_actual_source_undo':after_undo,'real_pointer_checkbox':True,'genuine_QColorDialog':dialog,'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','observer_sha256':hashlib.sha256(Path('scripts/original_field_border_ui_fixture.py').read_bytes()).hexdigest(),'scope':'Actual original composer dormant border property pointers/color dialog; actual source property Undo activates empty history and leaves properties unchanged; original export dialog not claimed'}
  (out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
 finally:
  if app:
   try:os.killpg(app.pid,signal.SIGTERM)
   except ProcessLookupError:pass
   app.wait(timeout=5)
  server.terminate();server.communicate(timeout=5)
