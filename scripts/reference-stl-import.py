#!/usr/bin/env python3
"""Actual unchanged STL command/dialog/worker/history; controlled chooser provider."""
import os,sys,json,ast,struct,traceback,math
from pathlib import Path
root=Path(sys.argv[1]).resolve()
for kind in ["CONFIG","DATA","CACHE"]:os.environ["XDG_"+kind+"_HOME"]=str(root/("xdg-"+kind.lower()))
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication,QDialog,QComboBox,QDialogButtonBox
from PySide6.QtCore import QTimer,QSettings,Qt
from PySide6.QtTest import QTest
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.main_window import MainWindow
from core.mesh import Mesh
from core.group import Group
from core.layers import Layer
from formats import stl,igz
from core.i18n import tr
import views.main_window as main
root=Path(sys.argv[1]).resolve();root.mkdir(parents=True,exist_ok=True)
assert not(root/'original-stl.json').exists(),"source already frozen"
app=QApplication([]);app.setOrganizationName('STLProof');app.setApplicationName('STLProof')
QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,str(root/'settings'))
tree=ast.parse(Path('scripts/reference-nested-import.py').read_text());names={'vec','mesh_state','shape','seed'}
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name in names],type_ignores=[]),'nested-observation','exec'))
def binary(name,triangles,solid=False,tail=b'',normal=(0,0,0)):
 header=(b'solid fixture'if solid else b'native fixture').ljust(80,b' ')
 data=header+struct.pack('<I',len(triangles))+b''.join(struct.pack('<12fH',*normal,*sum((tuple(p)for p in t),()),0)for t in triangles)+tail
 (root/name).write_bytes(data)
 return name
cubepts=[(0,0,0),(1000,0,0),(1000,1000,0),(0,1000,0),(0,0,1000),(1000,0,1000),(1000,1000,1000),(0,1000,1000)]
quads=[(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)]
cube=[tuple(cubepts[i]for i in t)for a,b,c,d in quads for t in[(a,b,c),(a,c,d)]]
binary('cube.stl',cube);binary('solid-binary.stl',cube,True);binary('trailing.stl',cube,tail=b'private metadata');binary('ignored-normal.stl',cube,normal=(float('nan'),0,0))
tilt=[((0,0,0),(1000,0,1000),(1000,1000,2000)),((0,0,0),(1000,1000,2000),(0,1000,1000))];binary('tilted.stl',tilt)
large=[((i*3000,0,0),(i*3000+1000,0,0),(i*3000,1000,0))for i in range(401)];binary('large.stl',large)
grid=[((x*1000,y*1000,0),((x+1)*1000,y*1000,0),((x+1)*1000,(y+1)*1000,0))for x in range(21)for y in range(10)]+[((x*1000,y*1000,0),((x+1)*1000,(y+1)*1000,0),(x*1000,(y+1)*1000,0))for x in range(21)for y in range(10)];grid3=grid+[((x,50000,z), (a,50000,b), (c,50000,d))for (x,z,_),(a,b,_),(c,d,_) in grid]+[((50000,y,z),(50000,a,b),(50000,c,d))for(y,z,_),(a,b,_),(c,d,_)in grid]
binary('grid.stl',grid3)
binary('nonfinite.stl',[((float('nan'),0,0),(1,0,0),(0,1,0))])
binary('degenerate.stl',[((0,0,0),(0,0,0),(0,0,0))]+cube)
(root/'malformed.stl').write_bytes(b'facet normal 0 0 1\nouter loop\nvertex 0 0 0\n')
(root/'ascii.stl').write_text('\ufeffSOLID example\r\n# comment\r\n'+''.join('FACET normal 0 0 1\r\n OUTER LOOP\r\n'+''.join(' VeRtEx %s %s %s\r\n'%p for p in t)+' ENDLOOP\r\n ENDFACET\r\n'for t in cube)+'ENDSOLID example\r\n')
states=[];cases=[];errors=[];timers=[];parses=[]
def exception(t,e,tb):
 errors.append({'type':t.__name__,'message':str(e),'traceback':''.join(traceback.format_exception(t,e,tb))});(root/'callback-errors.json').write_text(json.dumps(errors,indent=2)+'\n')
sys.excepthook=exception
original_parse=stl.parse_stl
def tracked(*a,**k):parses.append(str(a[0]));return original_parse(*a,**k)
stl.parse_stl=tracked
prefs=lambda:{k:QSettings().value(k,None)for k in['import/stl_unit','import/stl_simplify_mode']}
def observe(w,name,phase,outcome):
 states.append({'case':name,'phase':phase,'scene':shape(w.viewport.scene),'undo':len(w.viewport.history.undo_stack),'redo':len(w.viewport.history.redo_stack),'preferences':prefs(),'outcome':outcome,'camera':{'distance':w.viewport.camera.distance}})
 (root/'original-stl.partial.json').write_text(json.dumps({'cases':cases,'states':states,'callback_errors':errors},indent=2)+'\n')
def dialog(unit,mode,cancel):
 timer=QTimer();timers.append(timer);expired=[False];complete=[False]
 def poll():
  d=app.activeModalWidget()
  if not isinstance(d,QDialog)or d.windowTitle()!=tr('Import STL'):return
  boxes=d.findChildren(QComboBox)
  if len(boxes)!=2:return
  try:
   if cancel or expired[0]:d.reject()
   else:
    boxes[0].setCurrentIndex(['m','cm','mm','in','ft'].index(unit));boxes[1].setCurrentIndex(['none','principal','all'].index(mode))
    box=d.findChild(QDialogButtonBox);assert box is not None;QTest.mouseClick(box.button(QDialogButtonBox.Ok),Qt.LeftButton)
  except BaseException:exception(*sys.exc_info());d.reject()
  finally:complete[0]=True;timer.stop()
 def deadline():
  if not complete[0]:expired[0]=True;errors.append({'type':'FixtureTimeout','message':'STL dialog did not complete','traceback':''})
 timer.timeout.connect(poll);timer.start(10);QTimer.singleShot(5000,deadline)
critical=[];oldcritical=main.QMessageBox.critical
main.QMessageBox.critical=lambda *a,**k:critical.append(str(a[2]))
oldpicker=main.file_dialogs.getOpenFileName
recipes=[('default-cube','cube.stl','root','mm','principal','accepted'),('none-cube','cube.stl','root','mm','none','accepted'),('tilted-principal','tilted.stl','root','mm','principal','accepted'),('tilted-all','tilted.stl','root','mm','all','accepted'),('flat-large-editable','grid.stl','root','mm','principal','accepted'),('large-root','large.stl','root','mm','none','accepted'),('large-nested','large.stl','leaf','mm','none','accepted'),('small-nested','cube.stl','leaf','mm','principal','accepted'),('unit-cancel','cube.stl','root','ft','all','cancel'),('chooser-cancel','cube.stl','root','mm','principal','cancel'),('parse-failure','malformed.stl','root','cm','all','failed')]
for name,file,context,unit,mode,outcome in recipes:
 w=MainWindow();w.show();app.processEvents();seed(w.viewport.scene);igz.save_scene(w.viewport.scene,root/(name+'-input.igz'))
 if context=='leaf':w.viewport.begin_group_edit(w.viewport.scene.groups[0]);w.viewport.begin_group_edit(w.viewport.scene.edit_group.children[0])
 observe(w,name,'before','idle');before=shape(w.viewport.scene);history=(len(w.viewport.history.undo_stack),len(w.viewport.history.redo_stack));camera=w.viewport.camera.distance;beforeprefs=prefs();count=len(parses)
 main.file_dialogs.getOpenFileName=lambda *a,**k:('', '')if name=='chooser-cancel'else(str(root/file),'STL')
 if name!='chooser-cancel':dialog(unit,mode,name=='unit-cancel')
 w._on_import_stl();app.processEvents();observe(w,name,'import',outcome)
 assert w.viewport.camera.distance==camera,'STL unexpectedly changed camera'
 saved=None
 if outcome=='accepted':
  assert shape(w.viewport.scene)!=before and len(w.viewport.history.undo_stack)==history[0]+1,'real import/history missing'
  assert len(parses)==count+1,'parse-once worker violation'
  w.viewport.history.undo();observe(w,name,'undo','accepted');assert shape(w.viewport.scene)==before,'Undo changed original owner'
  w.viewport.history.redo();observe(w,name,'redo','accepted')
  while w.viewport.scene._edit_stack:w.viewport.end_group_edit()
  saved=name+'-saved.igz';igz.save_scene(w.viewport.scene,root/saved)
 else:
  assert shape(w.viewport.scene)==before and (len(w.viewport.history.undo_stack),len(w.viewport.history.redo_stack))==history,'failed/cancel import mutated model/history'
  assert len(parses)==count+(outcome=='failed'),'cancel unexpectedly parsed'
  if outcome=='cancel':assert prefs()==beforeprefs,'cancel changed persisted preferences'
  else:assert critical and prefs()=={'import/stl_unit':unit,'import/stl_simplify_mode':mode},'accepted failure did not preserve settings/error'
 cases.append({'name':name,'input':name+'-input.igz','stl':file,'context':context,'unit':unit,'simplify':mode,'outcome':outcome,'saved':saved,'phases':[s['phase']for s in states if s['case']==name]})
 (root/'completed-cases.json').write_text(json.dumps({'cases':cases,'states':states},indent=2)+'\n');w.hide();w.deleteLater();app.processEvents()
main.file_dialogs.getOpenFileName=oldpicker;main.QMessageBox.critical=oldcritical;stl.parse_stl=original_parse
controls=[]
for name,file,scale,mode in [('solid-binary','solid-binary.stl',.001,'principal'),('trailing','trailing.stl',.001,'principal'),('ignored-normal','ignored-normal.stl',.001,'principal'),('ASCII-BOM-CR','ascii.stl',.001,'principal'),('degenerate','degenerate.stl',.001,'principal'),('nonfinite-coordinate','nonfinite.stl',.001,'principal'),('invalid-scale','cube.stl',0,'principal'),('invalid-mode','cube.stl',.001,'unknown')]:
 try:
  mesh=stl.parse_stl(root/file,scale=scale,simplify_mode=mode);controls.append({'name':name,'file':file,'scale':scale,'simplify':mode,'outcome':'accepted','mesh':mesh_state(mesh)})
 except ValueError as e:controls.append({'name':name,'file':file,'scale':scale,'simplify':mode,'outcome':'refused','source_diagnostic':str(e)})
assert not errors,errors
(root/'original-stl.json').write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','cases':cases,'states':states,'decoder_controls':controls,'callback_errors':errors,'scope':'Actual original STL command, Qt option buttons, mesh-only worker, history and save. Controlled chooser and failure-message provider; camera distance/source errors diagnostic, unchanged camera operative.'},indent=2)+'\n')
print(json.dumps({'cases':len(cases),'states':len(states),'controls':len(controls)}))
