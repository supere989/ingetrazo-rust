#!/usr/bin/env python3
"""Actual pinned SKP App chooser, file worker, source merge/history/save."""
import os,sys,json,ast,io,zipfile,struct,hashlib,traceback,subprocess,shutil
from pathlib import Path
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C','/work/ingetrazo','rev-parse','HEAD'],text=True).strip()==PIN
contract=json.loads(Path('docs/audits/skp-import/fixture-contract.json').read_text())
for row in contract['original_sources']:
 assert hashlib.sha256((Path('/work/ingetrazo')/row['path']).read_bytes()).hexdigest()==row['sha256'],row['path']
for row in contract['sources']:
 assert hashlib.sha256(Path(row['path']).read_bytes()).hexdigest()==row['sha256'],row['path']
root=Path(sys.argv[1]).resolve();root.mkdir(parents=True,exist_ok=True)
assert not(root/'original-skp.json').exists(),'frozen output exists'
for kind in ['CONFIG','DATA','CACHE']:os.environ['XDG_'+kind+'_HOME']=str(root/('xdg-'+kind.lower()))
oracle=Path('/tmp/ingetrazo-openskp-reference')
assert subprocess.check_output(['git','-C',str(oracle),'rev-parse','HEAD'],text=True).strip()==contract['parser_pin']
for row in contract['parser_sources']:assert hashlib.sha256((oracle/row['path']).read_bytes()).hexdigest()==row['sha256'],row['path']
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path[:0]=[str(oracle/'packages/python/src'),'/work/ingetrazo']
import openskp
assert Path(openskp.__file__).resolve().is_relative_to((oracle/'packages/python/src').resolve())
(root/'parser-identity.json').write_text(json.dumps({'module':openskp.__file__,'pin':contract['parser_pin'],'sources':contract['parser_sources']},indent=2))
from PySide6.QtWidgets import QApplication,QMessageBox
from PySide6.QtCore import QSettings,QTimer
from PySide6.QtGui import QImage,QColor,QVector3D as V,QMatrix4x4 as M
from views.main_window import MainWindow
from core.mesh import Mesh
from core.group import Group
from core.layers import Layer
from core.materials import Material
from core.textlabel import TextLabel
from core.saved_views import from_lookat
from formats import igz,skp
import views.main_window as main
from skp_import_observations import scene_state
app=QApplication([]);app.setOrganizationName('SkpImportProof');app.setApplicationName('SkpImportProof')
QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,str(root/'settings'))
seedtree=ast.parse(Path('scripts/reference-nested-import.py').read_text())
exec(compile(ast.Module(body=[n for n in seedtree.body if isinstance(n,ast.FunctionDef)and n.name=='seed'],type_ignores=[]),'retained-source-seed','exec'))
assets=root/'assets';assets.mkdir();asset_rows=[]
for row in contract['fixtures']:
 src=Path(row['path']);raw=src.read_bytes();assert hashlib.sha256(raw).hexdigest()==row['sha256']
 (assets/row['file']).write_bytes(raw);asset_rows.append(row)
# Source modern empty recognition requires authored empty records AND a flat
# non-null thumbnail; retain generated engineering bytes rather than claiming a
# real-user blank model/version fixture. The prefix comes from frozen VFF input.
base=(assets/'modern-hole.skp').read_bytes();offset=base.find(b'PK\x03\x04');assert offset>=0
archive=zipfile.ZipFile(io.BytesIO(base[offset:]));buf=io.BytesIO()
image=QImage(4,4,QImage.Format_RGBA8888);image.fill(QColor(230,230,230,255));assert image.save(str(assets/'empty-thumbnail.png'))
with zipfile.ZipFile(buf,'w',zipfile.ZIP_DEFLATED)as z:
 for name in archive.namelist():
  if name!='model.dat' and not name.startswith('materials/') and name!='meta/model_thumbnail.png':z.writestr(name,archive.read(name))
 z.writestr('model.dat',bytes.fromhex('F401')+struct.pack('<I',6)+bytes.fromhex('F601')+struct.pack('<I',0))
 z.writestr('meta/model_thumbnail.png',(assets/'empty-thumbnail.png').read_bytes())
(assets/'modern-empty.skp').write_bytes(base[:offset]+buf.getvalue())
(assets/'unreadable.skp').write_bytes(b'not a SketchUp file\x00\xff')
# Saved cameras use the actual pinned _parse_pages TLV grammar. Preserve the
# geometry/input bytes and add named perspective/parallel/duplicate pages only.
def tlv(tag,body):return bytes.fromhex(tag)+struct.pack('<I',len(body))+body
def page(name,eye,target,up,fov,parallel,height,hidden):
 camera=b''.join(tlv(tag,struct.pack('<3d',*value))for tag,value in[('BD34',eye),('BE34',target),('BF34',up)])+tlv('C434',struct.pack('<d',fov))+tlv('C234',bytes([0 if parallel else 1]))+tlv('C334',struct.pack('<d',height))
 return tlv('4871',tlv('546F',tlv('556F',name.encode()))+tlv('4A71',tlv('BC34',camera))+tlv('5071',b''.join(bytes([len(raw)])+raw for value in hidden for raw in[value.to_bytes(max(1,(value.bit_length()+7)//8),'little')])) )
model=archive.read('model.dat');assert model[:2]==bytes.fromhex('F401')and struct.unpack('<I',model[2:6])[0]==len(model)-6
pages=tlv('0702',tlv('606D',tlv('616D',page('Existing view',(10,20,30),(1,2,3),(0,0,1),35,False,0,[])+page('Imported perspective',(100,100,100),(0,0,0),(0,0,1),35,False,0,[2,999])+page('Imported plan',(0,0,100),(0,0,0),(0,1,0),45,True,200,[2]))))
viewbuf=io.BytesIO()
with zipfile.ZipFile(viewbuf,'w',zipfile.ZIP_DEFLATED)as z:
 for name in archive.namelist():z.writestr(name,tlv('F401',model[6:]+pages)if name=='model.dat'else archive.read(name))
(assets/'modern-scenes.skp').write_bytes(base[:offset]+viewbuf.getvalue())

recipes=[
 ('legacy-root','legacy-quad.skp','root','accepted'),
 ('modern-layer-preserve','modern-hole.skp','root','accepted'),
 ('saved-scenes','modern-scenes.skp','root','accepted'),
 ('shared-root','shared.skp','root','accepted'),
 ('nested-root-ownership','nested.skp','leaf','accepted'),
 ('leader-metadata','leader.skp','root','accepted'),
 ('image-leaf','image.skp','leaf','accepted'),
 ('style-adopt','styles.skp','root','accepted'),
 ('style-existing','styles.skp','root','accepted'),
 ('material-collision','modern-hole.skp','root','accepted'),
 ('repeat-import','modern-scenes.skp','root','accepted'),
 ('modern-empty','modern-empty.skp','root','empty'),
 ('legacy-empty-warning','legacy-empty.skp','root','unreadable'),
 ('unreadable-warning','unreadable.skp','root','unreadable'),
 ('chooser-cancel','legacy-quad.skp','root','cancel')]
assert [r[0]for r in recipes]==contract['case_names']
cases=[];states=[];errors=[];dialogs=[];parses=[]
def exception(t,e,tb):
 errors.append({'type':t.__name__,'message':str(e),'traceback':''.join(traceback.format_exception(t,e,tb))})
 (root/'callback-errors.json').write_text(json.dumps(errors,indent=2))
sys.excepthook=exception
original=skp.parse_skp
def tracked(*a,**kw):parses.append(str(a[0]));return original(*a,**kw)
skp.parse_skp=tracked
oldpicker=main.file_dialogs.getOpenFileName

def observe(w,name,phase,outcome):
 states.append({'case':name,'phase':phase,'scene':scene_state(w.viewport.scene),'undo':len(w.viewport.history.undo_stack),'redo':len(w.viewport.history.redo_stack),'depth':len(w.viewport.scene._edit_stack),'outcome':outcome,'camera':{'distance':w.viewport.camera.distance}})
 (root/'original-skp.partial.json').write_text(json.dumps({'cases':cases,'states':states,'callback_errors':errors},indent=2))

for name,file,context,outcome in recipes:
 w=MainWindow();w.show();app.processEvents();seed(w.viewport.scene)
 w.viewport.scene.saved_views.append(from_lookat('Existing view',[3,4,5],[0,0,0],[0,0,1]))
 if name=='modern-layer-preserve':
  layer=Layer('Hidden tag');layer.visible=True;layer.locked=True;w.viewport.scene.layers.append(layer)
 if name=='material-collision':w.viewport.scene.materials['Green']=Material('Green',(.9,.8,.7))
 if name=='style-existing':w.viewport.scene.back_face_color=(.9,.8,.7)
 igz.save_scene(w.viewport.scene,root/(name+'-input.igz'))
 if context=='leaf':
  w.viewport.begin_group_edit(w.viewport.scene.groups[0]);w.viewport.begin_group_edit(w.viewport.scene.edit_group.children[0])
  # A real pre-import command exercises parked-vs-live global preimage.
  from core.history import SnapshotImport
  def prior(s):
   s.text_labels.append(TextLabel(V(1,2,3),V(0,1,0),'Prior nested label'))
   if name=='nested-root-ownership':s.mesh.add_edge(V(20,1,0),V(21,1,0))
  w.viewport.history.execute(SnapshotImport(prior))
 before=scene_state(w.viewport.scene);history=(len(w.viewport.history.undo_stack),len(w.viewport.history.redo_stack));camera=w.viewport.camera.distance;count=len(parses)
 observe(w,name,'before','idle')
 main.file_dialogs.getOpenFileName=lambda *a,**kw:('','')if outcome=='cancel'else(str(assets/file),'SKP')
 timer=QTimer();timer.setInterval(10);deadline=QTimer();deadline.setSingleShot(True);timed_out=[];seen=[]
 def poll(_w=w,_name=name,_seen=seen,_outcome=outcome):
  modal=QApplication.activeModalWidget()
  if isinstance(modal,QMessageBox):
   try:
    assert _outcome=='unreadable' and modal.icon()==QMessageBox.Warning,'unexpected import error modal'
    _seen.append({'case':_name,'kind':'warning','title':modal.windowTitle(),'text':modal.text()})
   except BaseException as e:exception(type(e),e,e.__traceback__)
   finally:modal.accept()
 def timeout(_flag=timed_out):
  _flag.append(True);modal=QApplication.activeModalWidget()
  if isinstance(modal,QMessageBox):modal.reject()
 timer.timeout.connect(poll);deadline.timeout.connect(timeout);timer.start();deadline.start(5000)
 try:w._on_import_skp()
 finally:timer.stop();deadline.stop()
 app.processEvents();dialogs.extend(seen);assert not timed_out,'bounded modal deadline reached';assert not errors,errors
 observe(w,name,'import',outcome);assert w.viewport.camera.distance==camera
 saved=None
 if outcome in ['accepted','empty']:
  assert len(parses)==count+1 and len(w.viewport.history.undo_stack)==history[0]+1,'actual parse/history admission'
  imported=scene_state(w.viewport.scene)
  if outcome=='accepted':assert imported['groups']!=before['groups'],'geometry/root publication was a no-op'
  else:assert imported['groups']==before['groups'],'empty import created geometry'
  if name=='saved-scenes':assert [v['name']for v in imported['views']]==['Existing view','Imported perspective','Imported plan']and imported['views'][0]==before['views'][0], 'saved camera/name admission'
  if name=='material-collision':assert 'Green (2)'in imported['materials']and imported['materials']['Green']==before['materials']['Green']
  if name=='modern-layer-preserve':assert next(x for x in imported['layers']if x['name']=='Hidden tag')==next(x for x in before['layers']if x['name']=='Hidden tag')
  w.viewport.history.undo();observe(w,name,'undo',outcome)
  expected=dict(before);expected['materials']=imported['materials'];expected['back_face_color']=imported['back_face_color']
  assert scene_state(w.viewport.scene)==expected,'Undo ownership changed'
  w.viewport.history.redo();observe(w,name,'redo',outcome);assert scene_state(w.viewport.scene)==imported
  if name=='repeat-import':
   oldnames=set(imported['materials']);w._on_import_skp();app.processEvents();observe(w,name,'repeat',outcome)
   assert set(w.viewport.scene.materials)==oldnames and len(parses)==count+2 and len(w.viewport.history.undo_stack)==history[0]+2
   w.viewport.history.undo();observe(w,name,'repeat-undo',outcome);assert scene_state(w.viewport.scene)==imported
   w.viewport.history.redo();observe(w,name,'repeat-redo',outcome)
  while w.viewport.scene._edit_stack:w.viewport.end_group_edit()
  observe(w,name,'finish',outcome)
  if name=='nested-root-ownership':
   finished=scene_state(w.viewport.scene);finished_history=len(w.viewport.history.undo_stack)
   w.viewport.history.undo();observe(w,name,'finish-undo',outcome)
   assert len(w.viewport.history.undo_stack)==finished_history-1 and len(w.viewport.scene.groups)==len(finished['groups']),'finished import root owner/history'
   w.viewport.history.redo();observe(w,name,'finish-redo',outcome);assert scene_state(w.viewport.scene)==finished
  saved=name+'-saved.igz';igz.save_scene(w.viewport.scene,root/saved)
 else:
  assert scene_state(w.viewport.scene)==before and history==(len(w.viewport.history.undo_stack),len(w.viewport.history.redo_stack)),'failed/cancel import changed model/history'
  assert len(parses)==count+(outcome=='unreadable' and file!='unreadable.skp') and len(seen)==(outcome=='unreadable')
 cases.append({'name':name,'file':'assets/'+file,'input':name+'-input.igz','context':context,'outcome':outcome,'saved':saved,'phases':[s['phase']for s in states if s['case']==name],'parses':len(parses)-count})
 (root/'completed-cases.json').write_text(json.dumps({'cases':cases,'states':states},indent=2));w.hide();w.deleteLater();app.processEvents()
main.file_dialogs.getOpenFileName=oldpicker;skp.parse_skp=original
assert not errors,errors
(root/'original-skp.json').write_text(json.dumps({'source_pin':PIN,'cases':cases,'states':states,'dialogs_diagnostic':dialogs,'callback_errors':errors,'scope':contract['scope'],'parses':len(parses)},indent=2)+'\n')
print(json.dumps({'cases':len(cases),'states':len(states),'parses':len(parses)}))
