#!/usr/bin/env python3
"""Actual original DWG command/unit/history proof; controlled chooser answer only."""
import os,sys,json,ast,shutil,hashlib,subprocess,traceback
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication,QInputDialog,QComboBox,QDialogButtonBox
from PySide6.QtCore import QTimer,QSettings
from PySide6.QtTest import QTest
from PySide6.QtCore import Qt
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.main_window import MainWindow
from formats import dwg_bridge,igz
from core.group import Group
from core.mesh import Mesh
from core.layers import Layer
root=Path(sys.argv[1]).resolve();root.mkdir(parents=True,exist_ok=True)
assert not(root/'original-dwg.json').exists(),"source already frozen"
for n in ['entities-2d.dwg','entities-2d.scr','COPYING','upstream.json']:shutil.copyfile(Path(sys.argv[2])/n,root/n)
app=QApplication([]);app.setOrganizationName('DWGProof');app.setApplicationName('DWGProof')
QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,str(root/'settings'))
# Existing independently used source geometry observation, no renderer replacement.
tree=ast.parse(Path('scripts/reference-nested-import.py').read_text());names={'vec','mesh_state','shape','seed'}
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name in names],type_ignores=[]),'nested-observation','exec'))
states=[];cases=[];callback_errors=[];temps=[]
original_convert=dwg_bridge.dwg_to_dxf
def observed_convert(path):
 result=original_convert(path);temps.append(result);shutil.copyfile(result,root/'real-converted.dxf');return result
dwg_bridge.dwg_to_dxf=observed_convert
def report_exception(t,e,tb):
 callback_errors.append({"type":t.__name__,"message":str(e),"traceback":"".join(traceback.format_exception(t,e,tb))})
 (root/'callback-errors.json').write_text(json.dumps(callback_errors,indent=2)+'\n')
sys.excepthook=report_exception
def observe(w,name,phase,outcome):
 states.append({'case':name,'phase':phase,'scene':shape(w.viewport.scene),'undo':len(w.viewport.history.undo_stack),'redo':len(w.viewport.history.redo_stack),'outcome':outcome,'camera':{'distance':w.viewport.camera.distance}})
 (root/'original-dwg.partial.json').write_text(json.dumps({'cases':cases,'states':states,'callback_errors':callback_errors},indent=2)+'\n')
unit_timers=[]
def choose_unit(cancel=False):
 completed=[False];timed_out=[False];timer=QTimer();unit_timers.append(timer)
 def accept():
  d=app.activeModalWidget()
  if not isinstance(d,QInputDialog):return
  timer.stop()
  try:
   if cancel or timed_out[0]:d.reject()
   else:
    combo=d.findChild(QComboBox);assert combo is not None;combo.setCurrentIndex(2)
    box=d.findChild(QDialogButtonBox);assert box is not None;QTest.mouseClick(box.button(QDialogButtonBox.Ok),Qt.LeftButton)
  except BaseException:
   sys.excepthook(*sys.exc_info());d.reject()
  finally:completed[0]=True
 def timeout():
  if completed[0]:return
  timed_out[0]=True;callback_errors.append({'type':'FixtureTimeout','message':'unit chooser did not complete','traceback':''})
  d=app.activeModalWidget()
  if d is not None:d.reject()
 timer.timeout.connect(accept);timer.start(10);QTimer.singleShot(5000,timeout)
import views.main_window as main
original_picker=main.file_dialogs.getOpenFileName
for name,context,cancel in [('real-root','root',False),('real-nested','leaf',False),('unit-cancel','root',True),('chooser-cancel','root',False)]:
 w=MainWindow();w.show();app.processEvents();seed(w.viewport.scene)
 igz.save_scene(w.viewport.scene,root/(name+'-input.igz'))
 if context=='leaf':w.viewport.begin_group_edit(w.viewport.scene.groups[0]);w.viewport.begin_group_edit(w.viewport.scene.edit_group.children[0])
 observe(w,name,'before','idle');before=shape(w.viewport.scene);before_history=(len(w.viewport.history.undo_stack),len(w.viewport.history.redo_stack))
 main.file_dialogs.getOpenFileName=lambda *a,**k:('', '')if name=='chooser-cancel'else(str(root/'entities-2d.dwg'),'DWG')
 if name!='chooser-cancel':choose_unit(cancel)
 w._on_import_dwg();app.processEvents();observe(w,name,'import','cancel'if cancel or name=='chooser-cancel'else'accepted')
 saved=None
 if not(cancel or name=='chooser-cancel'):
  assert shape(w.viewport.scene)!=before,'actual converted geometry missing';assert len(w.viewport.history.undo_stack)==before_history[0]+1,'actual import history missing'
  w.viewport.history.undo();observe(w,name,'undo','accepted');w.viewport.history.redo();observe(w,name,'redo','accepted')
  while w.viewport.scene._edit_stack:w.viewport.end_group_edit()
  saved=name+'-original.igz';igz.save_scene(w.viewport.scene,root/saved)
 if cancel or name=='chooser-cancel':assert shape(w.viewport.scene)==before and (len(w.viewport.history.undo_stack),len(w.viewport.history.redo_stack))==before_history,'cancel mutated model/history'
 cases.append({'name':name,'input':name+'-input.igz','dwg':'entities-2d.dwg','context':context,'unit':'m','cancel':cancel,'saved':saved,'phases':[x['phase']for x in states if x['case']==name]})
 (root/'completed-cases.json').write_text(json.dumps({'cases':cases,'states':states},indent=2)+'\n')
 # Fixture teardown must not invoke the unrelated unsaved-document CloseEvent.
 w.hide();w.deleteLater();app.processEvents()
main.file_dialogs.getOpenFileName=original_picker
repairs=[]
raws={'null':b'0\nSECTION\n2\nBLOCKS\n0\nENDBLK\n5\n0\n0\nENDSEC\n0\nEOF',
'duplicate-seed':b'0\nSECTION\n2\nHEADER\n9\n$HANDSEED\n5\n2\n0\nENDSEC\n0\nSECTION\n2\nTABLES\n0\nBLOCK_RECORD\n5\n2\n0\nENDSEC\n0\nSECTION\n2\nOBJECTS\n0\nLAYOUT\n5\n2\n0\nENDSEC\n0\nEOF',
'control-crlf-tail':b'0\r\nSECTION\r\n2\r\nENTITIES\r\n0\r\nTEXT\r\n5\r\nA\r\n1\r\nCA\xd1ER\x0b\x85\x1c\r\n0\r\nTEXT\r\n5\r\nA\r\n1\r\nodd\r\nTAIL'}
for name,b in raws.items():
 f=root/(name+'.dxf');f.write_bytes(b);dwg_bridge._strip_null_handles(f);n=dwg_bridge._dedupe_handles(f);repairs.append({'name':name,'input_hex':b.hex(),'file':f.name,'duplicates':n})
assert not callback_errors,callback_errors
assert temps and all(not p.exists() and not p.parent.exists() for p in temps),'operative unit/import cleanup failed'
dwg_bridge.dwg_to_dxf=original_convert
# Discovery knobs are confined to this fixture, never the original files.
discovery=[];vendor=root/'discovery/checkouts/app/vendor/libredwg/bin';sibling=root/'discovery/ingecad/vendor/libredwg/bin';system=root/'discovery/path';
for p in [vendor,sibling,system]:p.mkdir(parents=True,exist_ok=True)
old_vendor,old_sibling,old_which,old_platform=dwg_bridge._VENDOR_BIN,dwg_bridge._SIBLING_BIN,dwg_bridge.shutil.which,dwg_bridge.sys.platform
dwg_bridge._VENDOR_BIN=vendor;dwg_bridge._SIBLING_BIN=sibling;dwg_bridge.shutil.which=lambda name:str(system/name)if(system/name).exists()else None
for p in [vendor,sibling,system]:(p/'dwg2dxf').write_bytes(b'fixture')
for name in ['vendor','sibling','PATH','missing']:
 found=dwg_bridge.find_dwg2dxf();expected={'vendor':vendor/'dwg2dxf','sibling':sibling/'dwg2dxf','PATH':system/'dwg2dxf','missing':None}[name];assert found==expected,(name,found,expected);discovery.append({'name':name,'result':name if found else None})
 if found:found.unlink()
(vendor/'dwg2dxf.exe').write_bytes(b'MZ');dwg_bridge.sys.platform='win32';discovery.append({'name':'windows-exe','result':dwg_bridge.find_dwg2dxf().name});dwg_bridge.sys.platform='linux';discovery.append({'name':'linux-no-exe','result':dwg_bridge.find_dwg2dxf()})
dwg_bridge._VENDOR_BIN, dwg_bridge._SIBLING_BIN,dwg_bridge.shutil.which,dwg_bridge.sys.platform=old_vendor,old_sibling,old_which,old_platform
converter_controls=[]
for name,code in [('warning-output',"from pathlib import Path;import sys;Path(sys.argv[1]).write_bytes(b'0\\nEOF');sys.stderr.buffer.write(b'CA\\xd1ERIAS\\n');sys.exit(7)"),('no-output',"import sys;sys.stderr.write('first\\nsecond\\nthird\\nfourth\\n');sys.exit(3)"),('timeout',"import time;time.sleep(1)")]:
 script=root/(name+'.py');script.write_text(code+'\n');output=root/(name+'.dxf');old_timeout=dwg_bridge._TIMEOUT
 if name=='timeout':dwg_bridge._TIMEOUT=.05
 try:
  warnings=dwg_bridge._run([sys.executable,str(script),str(output)],output);converter_controls.append({'name':name,'outcome':'accepted','warnings':warnings,'output_hex':output.read_bytes().hex()})
 except dwg_bridge.DwgBridgeError as e:converter_controls.append({'name':name,'outcome':'timeout'if name=='timeout'else'no-output','source_diagnostic':str(e),'deadline_seconds':.05 if name=='timeout'else 300})
 finally:dwg_bridge._TIMEOUT=old_timeout

(root/'original-dwg.json').write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','cases':cases,'states':states,'repairs':repairs,'discovery':discovery,'converter_controls':converter_controls,'temporary_cleanup':True,'callback_errors':callback_errors,'scope':'Actual MainWindow command with controlled file chooser provider; real Qt unit buttons; no GPU render claim.'},indent=2)+'\n')
print(json.dumps({'cases':len(cases),'states':len(states)}))
