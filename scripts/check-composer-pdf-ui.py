#!/usr/bin/env python3
"""Actual native composer PDF button clicks; genuine backend absence is a blocker."""
import os,json,subprocess,tempfile,time,select,ctypes as C,ctypes.util,shutil,hashlib
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")  # Reader stays headless; native Vulkan/XTEST use their owned X server.
text_source=os.environ.get("INGETRAZO_PDF_UI_FIELD_OBSERVATIONS") or os.environ.get("INGETRAZO_PDF_UI_TEXT_OBSERVATIONS")
field_mode=bool(os.environ.get("INGETRAZO_PDF_UI_FIELD_OBSERVATIONS"))
canvas_mode=os.environ.get('INGETRAZO_PDF_UI_CANVAS_FIELDS')=='1'
literal_control=os.environ.get('INGETRAZO_PDF_UI_CANVAS_LITERAL_CONTROL')=='1'
text_cases=json.loads(Path(text_source).read_text())["cases"] if text_source else None
def expected_canvas(case):return [case['expanded']]+([case['secondary']['expanded']]if case.get('secondary')else [])
unsupported=os.environ.get("INGETRAZO_PDF_UI_UNSUPPORTED")=="1"
out=Path(os.environ.get("INGETRAZO_PDF_UI_EVIDENCE","docs/audits/composer-pdf-ui"+("-unsupported" if unsupported else "")));out.mkdir(exist_ok=True);(out/"state.json").unlink(missing_ok=True)
def wait(fn):
 end=time.monotonic()+20
 while time.monotonic()<end:
  try:
   v=fn()
   if v:return v
  except (OSError,ValueError):pass
  time.sleep(.05)
 raise AssertionError("Actual UI timeout")
binary=(Path(os.environ["CARGO_TARGET_DIR"])/"debug/examples/composer_pdf_ui_fixture").resolve()
assert shutil.which("zenity"),"Genuine Zenity backend required"
with tempfile.TemporaryDirectory() as runtime:
 server=subprocess.Popen(["Xvfb","-displayfd","1","-screen","0","1600x1000x24","-nolisten","tcp"],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);app=None
 try:
  assert select.select([server.stdout],[],[],10)[0];display=":"+server.stdout.readline().strip()
  env=os.environ.copy();env.update(DISPLAY=display,XDG_RUNTIME_DIR=runtime,VK_DRIVER_FILES="/usr/share/vulkan/icd.d/lvp_icd.json",INGETRAZO_VULKAN_DEVICE="llvmpipe",INGETRAZO_PDF_UI_OBSERVE="1",INGETRAZO_PDF_UI_OUT=str(out.resolve()),INGETRAZO_PDF_UI_RECIPES=str(Path("docs/audits/sheet-shapes-vector-pdf/recipes.json").resolve()));env.pop("WAYLAND_DISPLAY",None)
  log=open(out/"runtime.log","w");app=subprocess.Popen([str(binary)],env=env,stdout=log,stderr=log,start_new_session=True)
  state=lambda:json.loads((out/"state.json").read_text());initial=wait(state)
  x=C.CDLL(ctypes.util.find_library("X11"));x.XOpenDisplay.argtypes=[C.c_char_p];x.XOpenDisplay.restype=C.c_void_p;d=x.XOpenDisplay(display.encode());assert d
  x.XDefaultRootWindow.argtypes=[C.c_void_p];x.XDefaultRootWindow.restype=C.c_ulong;root=x.XDefaultRootWindow(d)
  x.XQueryTree.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ulong)),C.POINTER(C.c_uint)]
  children=C.POINTER(C.c_ulong)();r,p,n=C.c_ulong(),C.c_ulong(),C.c_uint()
  def mapped_windows():
   x.XQueryTree(d,root,C.byref(r),C.byref(p),C.byref(children),C.byref(n));return n.value>0
  wait(mapped_windows)
  x.XGetGeometry.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_uint),C.POINTER(C.c_uint),C.POINTER(C.c_uint),C.POINTER(C.c_uint)]
  class Attributes(C.Structure):
   _fields_=[(name,C.c_int) for name in ['x','y','width','height','border_width','depth']]+[('visual',C.c_void_p),('root',C.c_ulong)]+[(name,C.c_int) for name in ['window_class','bit_gravity','win_gravity','backing_store']]+[('backing_planes',C.c_ulong),('backing_pixel',C.c_ulong),('save_under',C.c_int),('colormap',C.c_ulong),('map_installed',C.c_int),('map_state',C.c_int),('all_event_masks',C.c_long),('your_event_mask',C.c_long),('do_not_propagate_mask',C.c_long),('override_redirect',C.c_int),('screen',C.c_void_p)]
  x.XGetWindowAttributes.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(Attributes)]
  x.XGetTransientForHint.argtypes=[C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong)]
  def mapped_confirmation(wid,parent):
   attrs=Attributes();owner=C.c_ulong()
   return bool(x.XGetWindowAttributes(d,wid,C.byref(attrs)) and attrs.map_state==2 and not attrs.override_redirect and x.XGetTransientForHint(d,wid,C.byref(owner)) and owner.value==parent)
  def area(wid):
   rr,xx,yy,w,h,border,depth=C.c_ulong(),C.c_int(),C.c_int(),C.c_uint(),C.c_uint(),C.c_uint(),C.c_uint();x.XGetGeometry(d,wid,C.byref(rr),C.byref(xx),C.byref(yy),C.byref(w),C.byref(h),C.byref(border),C.byref(depth));return w.value*h.value
  initial_windows={children[i] for i in range(n.value)};window=max(initial_windows,key=area)
  x.XWarpPointer.argtypes=[C.c_void_p,C.c_ulong,C.c_ulong,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_int,C.c_int];x.XFlush.argtypes=[C.c_void_p];x.XSetInputFocus.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_ulong];x.XSetInputFocus(d,window,1,0)
  xt=C.CDLL(ctypes.util.find_library("Xtst"));xt.XTestFakeButtonEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
  def click(point,target_window=None):
   x.XWarpPointer(d,0,target_window or window,0,0,0,0,round(point[0]),round(point[1]));x.XFlush(d);time.sleep(.1)
   for down in (1,0):assert xt.XTestFakeButtonEvent(d,1,down,0);x.XFlush(d);time.sleep(.15)
  def capture(name):subprocess.run(["import","-display",display,"-window",str(window),str(out/(name+".png"))],check=True)
  x.XKeysymToKeycode.argtypes=[C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype=C.c_uint
  xt.XTestFakeKeyEvent.argtypes=[C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
  def key(symbol,modifier=None):
   if modifier:xt.XTestFakeKeyEvent(d,x.XKeysymToKeycode(d,modifier),1,0)
   code=x.XKeysymToKeycode(d,symbol);assert code;xt.XTestFakeKeyEvent(d,code,1,0);xt.XTestFakeKeyEvent(d,code,0,0)
   if modifier:xt.XTestFakeKeyEvent(d,x.XKeysymToKeycode(d,modifier),0,0)
   x.XFlush(d);time.sleep(.02)
  def dialog_save(path,label):
   def dialog():
    cs=C.POINTER(C.c_ulong)();nn=C.c_uint();x.XQueryTree(d,root,C.byref(r),C.byref(p),C.byref(cs),C.byref(nn));return next((cs[i]for i in range(nn.value)if cs[i]not in initial_windows and area(cs[i])>10000),None)
   dialog_window=wait(dialog);time.sleep(.4)
   subprocess.run(["import","-display",display,"-window",str(dialog_window),str(out/(label+"-dialog.png"))],check=True)
   x.XSetInputFocus(d,dialog_window,1,0);key(ord('a'),0xffe3)
   for char in str(path):
    key(ord(char),0xffe1 if char in '_:ABCDEFGHIJKLMNOPQRSTUVWXYZ' else None)
   subprocess.run(['import','-display',display,'-window',str(dialog_window),str(out/(label+'-typed-dialog.png'))],check=True)
   key(0xff0d)
   if unsupported:
    def overwrite_confirmation():
     cs=C.POINTER(C.c_ulong)();nn=C.c_uint();x.XQueryTree(d,root,C.byref(r),C.byref(p),C.byref(cs),C.byref(nn));return next((cs[i] for i in range(nn.value) if cs[i] not in initial_windows and cs[i]!=dialog_window and area(cs[i])>10000 and mapped_confirmation(cs[i],dialog_window)),None)
    confirm=wait(overwrite_confirmation)
    subprocess.run(['import','-display',display,'-window',str(confirm),str(out/(label+'-overwrite-dialog.png'))],check=True)
    attrs=Attributes();assert x.XGetWindowAttributes(d,confirm,C.byref(attrs))
    x.XSetInputFocus(d,confirm,1,0)
    click((attrs.width*0.75,attrs.height-24),confirm)
  border_gestures=None
  if field_mode:
   before=state();assert before["border"]["on"] is False and before["border"]["color"]=="#1e242c"
   canvas_before=None
   if canvas_mode:
    if not literal_control:
     expected=json.loads(Path(text_source).read_text())['before_equal_second']
     click(before['sheet_tabs'][1]);canvas_before=wait(lambda:state()if state()['active_sheet']==1 and state()['canvas_text']==[expected]+expected_canvas(text_cases[1])[1:]else None);capture('canvas-equal-second-before')
     count=canvas_before['field_index_computations'];expansions=canvas_before['field_expansion_computations'];assert canvas_before['calendar_samples_per_frame']==1;time.sleep(.15);idle=state();assert idle['field_index_computations']==count and idle['field_expansion_computations']==expansions and idle['calendar_samples_per_frame']==1
     click(canvas_before['sheet_tabs'][0]);wait(lambda:state()['active_sheet']==0)
   click(before["border_menu"]);controls=wait(lambda:state().get("border_controls"));capture("border-menu-before")
   click(controls[0]);wait(lambda:state()["border"]["on"] is True)
   click(controls[1]);key(ord('a'),0xffe3)
   for char in '#ffffff':key(ord(char),0xffe1 if char=='#' else None)
   key(0xff0d);wait(lambda:state()["border"]["color"]=='#ffffff')
   capture("border-white-enabled")
   click(controls[0]);wait(lambda:state()["border"]["on"] is False)
   key(0xff1b);time.sleep(.15);capture("border-white-disabled")
   edited=state();assert edited["border"]["color"]=='#ffffff' and edited["authored_text"]==before["authored_text"]
   assert edited['sheet_history']==[0,0]
   key(ord('z'),0xffe3);undone=wait(lambda:state()if any('key: Z' in e['event']for e in state()['history_key_events'])else None);capture("border-undo")
   key(ord('y'),0xffe3);redone=wait(lambda:state()if any('key: Y' in e['event']for e in state()['history_key_events'])else None);capture("border-redo")
   assert undone['border']==redone['border']==edited['border']and redone['sheet_history']==[0,0]
   assert any('key: Z' in event['event'] and 'ctrl: true' in event['event'] and event['final_frame_modifiers']=='Modifiers::NONE'for event in redone['history_key_events'])
   assert any('key: Y' in event['event'] and 'ctrl: true' in event['event'] and event['final_frame_modifiers']=='Modifiers::NONE'for event in redone['history_key_events'])
   # Genuine item Add/Undo/Redo with later direct paper edits. Remove it before PDF export.
   initial_items=redone['item_count'];click(redone['add_north']);added=wait(lambda:state()if state()['item_count']==initial_items+1 and state()['sheet_history']==[1,0]else None);capture('item-added')
   key(ord('z'),0xffe3);item_undo=wait(lambda:state()if state()['item_count']==initial_items and state()['sheet_history']==[0,1]else None);capture('item-undo')
   assert item_undo['border']==edited['border']
   key(ord('y'),0xffe3);item_redo=wait(lambda:state()if state()['item_count']==initial_items+1 and state()['sheet_history']==[1,0]else None);capture('item-redo');assert item_redo['border']==edited['border']
   key(ord('z'),0xffe3);redone=wait(lambda:state()if state()['item_count']==initial_items and state()['sheet_history']==[0,1]else None)
   canvas_states=[]
   if canvas_mode:
    for i in [1,2,0]:
     click(state()['sheet_tabs'][i]);observed=wait(lambda:state()if state()['active_sheet']==i and state()['canvas_text']==expected_canvas(text_cases[i])else None);capture('canvas-sheet-'+str(i+1));canvas_states.append(observed)
     count=observed['field_index_computations'];expansions=observed['field_expansion_computations'];assert observed['calendar_samples_per_frame']==1;time.sleep(.15);idle=state();assert idle['field_index_computations']==count and idle['field_expansion_computations']==expansions and idle['calendar_samples_per_frame']==1
    redone=state()
   border_gestures={"before":before,"edited":edited,"undo":undone,"redo":redone,"item_add":added,"item_undo":item_undo,"item_redo":item_redo,"canvas_before":canvas_before,"canvas_states":canvas_states,"actual_pointer_properties":True,"actual_global_shortcuts_route_sheet_history":True}
  if os.environ.get("INGETRAZO_PDF_UI_CANVAS_ONLY")=="1":
   assert canvas_mode and border_gestures
   report={"border_gestures":border_gestures,"binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"scope":"Actual native ash/XTEST canvas-only replay; no PDF export replay."};(out/"report.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report));raise SystemExit(0)
  current_path=out.resolve()/"current.pdf";all_path=out.resolve()/"all.pdf"
  if unsupported:
   current_path.write_bytes(b"existing destination current");all_path.write_bytes(b"existing destination all")
  else:
   current_path.unlink(missing_ok=True);all_path.unlink(missing_ok=True)
  capture("before");click(initial["current_button"]);dialog_save(current_path,"current");current=wait(lambda:state() if state()["current_clicks"]==1 else None);capture("current-click")
  if unsupported:
   current=wait(lambda:state() if state()["error_dialog"] and state()["error_ok"] else None);time.sleep(.15);assert state()["error_dialog"]==current["error_dialog"];capture("current-error-dialog")
   click(current["error_ok"]);wait(lambda:state() if state()["error_dialog"] is None else None)
  click(current["all_button"]);dialog_save(all_path,"all");atlas=wait(lambda:state() if state()["all_clicks"]==1 else None);capture("all-click")
  if unsupported:
   atlas=wait(lambda:state() if state()["error_dialog"] and state()["error_ok"] else None);time.sleep(.15);assert state()["error_dialog"]==atlas["error_dialog"];capture("all-error-dialog")
   click(atlas["error_ok"]);dismissed=wait(lambda:state() if state()["error_dialog"] is None else None);capture("error-dismissed")
   from PySide6.QtGui import QImage
   error_image=QImage(str(out/"all-error-dialog.png"));quiet_image=QImage(str(out/"error-dismissed.png"));assert not error_image.isNull()and not quiet_image.isNull()
   left=round(atlas["error_ok"][0]-20);top=round(atlas["error_ok"][1]-70)
   def dark(image):return sum(max(image.pixelColor(x,y).red(),image.pixelColor(x,y).green(),image.pixelColor(x,y).blue())<100 for x in range(left,left+300)for y in range(top,top+90))
   popup_dark=dark(error_image);quiet_dark=dark(quiet_image);assert popup_dark>10000 and quiet_dark<500,(popup_dark,quiet_dark)
  warm=None
  if field_mode:
   assert current['scene_signature']==atlas['scene_signature']==border_gestures['redo']['scene_signature']
   warm_path=out.resolve()/'warm-all.pdf';warm_path.unlink(missing_ok=True)
   click(atlas['all_button']);dialog_save(warm_path,'warm-all');warm=wait(lambda:state()if state()['all_clicks']==2 else None);capture('warm-all-click')
   assert warm_path.read_bytes()==all_path.read_bytes()
   assert warm['scene_signature']==atlas['scene_signature'] and warm['paper_resources']==atlas['paper_resources']
  if unsupported:
   assert atlas["sheets"]==initial["sheets"]==1
   assert current_path.read_bytes()==b"existing destination current" and all_path.read_bytes()==b"existing destination all"
   assert current["export_result"]==atlas["export_result"]=="PDF raster/perspective view painter remains unavailable; orthographic vectorial views are supported",atlas
   counts=[]
  else:
   assert atlas["sheets"]==initial["sheets"]==(len(text_cases) if text_cases else 9)
   assert current_path.read_bytes().startswith(b"%PDF-") and all_path.read_bytes().startswith(b"%PDF-")
   from PySide6.QtWidgets import QApplication
   from PySide6.QtPdf import QPdfDocument
   from PySide6.QtGui import QImage,QPainter
   from PySide6.QtCore import QSize
   qt=QApplication([]);counts=[];ink=[];first_pages=[]
   for path in [current_path,all_path]:
    doc=QPdfDocument();assert doc.load(str(path))==QPdfDocument.Error.None_;counts.append(doc.pageCount())
    for page in range(doc.pageCount()):
     raw=doc.render(page,QSize(842,595));assert not raw.isNull();image=QImage(raw.size(),QImage.Format.Format_RGB32);image.fill(0xffffffff);painter=QPainter(image);painter.drawImage(0,0,raw);painter.end();image=image.convertToFormat(QImage.Format.Format_RGBA8888);data=bytes(image.constBits());nonwhite=sum(data[i:i+3]!=b'\xff\xff\xff'for i in range(0,len(data),4));assert nonwhite>100;ink.append(nonwhite)
     if page==0:first_pages.append(data)
   assert first_pages[0]==first_pages[1]
   assert counts==[1,len(text_cases) if text_cases else 9]
   if text_cases:
    source_dir=Path(text_source).parent
    for path in [current_path,all_path]:
     native=QPdfDocument();assert native.load(str(path))==QPdfDocument.Error.None_
     for index in range(native.pageCount()):
      name=text_cases[index]["name"];original=QPdfDocument();assert original.load(str(source_dir/(name+".original.pdf")))==QPdfDocument.Error.None_
      assert native.pagePointSize(index)==original.pagePointSize(0),(name,"Actual PDF page dimensions differ from source")
      assert native.getAllText(index).text()==original.getAllText(0).text(),name
      for dpi in (72,144,300):
       size=original.pagePointSize(0);pixels=QSize(round(size.width()*dpi/72),round(size.height()*dpi/72))
       def white(doc,page):
        raw=doc.render(page,pixels);assert not raw.isNull();im=QImage(raw.size(),QImage.Format.Format_RGB32);im.fill(0xffffffff);p=QPainter(im);p.drawImage(0,0,raw);p.end();return bytes(im.constBits())
       assert white(native,index)==white(original,0),(name,dpi,"Actual button/dialog PDF differs from frozen source")
  report={"warm_dialog_export":warm,"border_gestures":border_gestures,"text_frozen_source_cases":len(text_cases) if text_cases else None,"text_source_exact_pages":(len(text_cases)+1)*3 if text_cases else None,"text_reader_exact_pages":len(text_cases)+1 if text_cases else None,"actual_x11_pointer_buttons_passed":True,"current":current,"all":atlas,"dialog_acceptance":"Actual genuine Zenity GTK save dialogs entered with XTEST keyboard","successful_ui_pdf_export":not unsupported,"pdf_page_counts":counts,"page_nonwhite_pixels":[] if unsupported else ink,"error_dialog_dismissed":dismissed if unsupported else None,"error_popup_dark_pixels":popup_dark if unsupported else None,"dismissed_popup_dark_pixels":quiet_dark if unsupported else None,"unsupported_destination_ui":"Actual default sheet current/all errors preserve confirmed overwrite destinations" if unsupported else "Separately exercised in unsupported scenario","binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"scope":"Native ash production composer button dispatch on isolated Xvfb/llvmpipe; instrumentation observes actual response rects/clicks only."};(out/"report.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report))
 finally:
  if app:
   import signal
   try:os.killpg(app.pid,signal.SIGTERM)
   except ProcessLookupError:pass
   try:app.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(app.pid,signal.SIGKILL);app.wait(timeout=5)
  server.terminate();server.communicate(timeout=5)
