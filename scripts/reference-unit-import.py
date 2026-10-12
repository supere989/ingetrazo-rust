#!/usr/bin/env python3
"""Only missing actual OBJ/DXF chooser/unit/face cleanup App workflows."""
import ast,hashlib,json,os,subprocess,sys,traceback,zipfile
from pathlib import Path
repo=Path(__file__).resolve().parents[1];root=Path(sys.argv[1]).resolve();root.mkdir(parents=True,exist_ok=True)
assert not (root/'original-unit-import.json').exists()
for key,name in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[key]=str(root/name)
os.environ['QT_QPA_PLATFORM']='offscreen';original=(repo/'ingetrazo').resolve();PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()==PIN
sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication,QInputDialog,QComboBox,QDialogButtonBox,QMessageBox
from PySide6.QtCore import QTimer,QSettings,Qt
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from PySide6.QtTest import QTest
from views.main_window import MainWindow
from formats import igz
from core.group import Group
from core.mesh import Mesh
from core.layers import Layer
from exchange_import_observations import scene_state
import views.main_window as main
import ezdxf
app=QApplication([]);app.setOrganizationName('UnitImportProof');app.setApplicationName('UnitImportProof');QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,str(root/'settings'))
tree=ast.parse((repo/'scripts/reference-nested-import.py').read_text());exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='seed'],type_ignores=[]),'retained-seed','exec'))
inputs=root/'assets';inputs.mkdir();reused=[]
for folder,names in [('obj-mtl-import',['color-root.obj','color-root.mtl','windows-options-leaf.obj','windows-options-leaf.mtl','paint with spaces.png']),('dxf-blocks',['shared-leaf.dxf'])]:
 archive=repo/'docs/audits'/folder/'source-records.zip';manifest=json.loads((archive.parent/'source-manifest.json').read_text());assert hashlib.sha256(archive.read_bytes()).hexdigest()==manifest['archive_sha256']
 with zipfile.ZipFile(archive)as z:
  for name in names:
   b=z.read(name);(inputs/name).write_bytes(b);reused.append({'archive':str(archive.relative_to(repo)),'archive_sha256':manifest['archive_sha256'],'member':name,'sha256':hashlib.sha256(b).hexdigest()})
for kind in ['3dface','solid','trace','face-block']:
 d=ezdxf.new('R2010');d.units=6;m=d.modelspace();d.layers.new('Faces')
 if kind=='face-block':m=d.blocks.new('FacesBlock')
 # Three triangles: stitched square and a T-junction edge witness. Keep free linework.
 for loop in [[(0,0,0),(4,0,0),(2,2,0)],[(4,0,0),(4,4,0),(2,2,0)],[(4,4,0),(0,4,0),(0,0,0),(2,2,0)]]:
  points=loop if len(loop)==4 else loop+[loop[-1]]
  if kind in ['3dface','face-block']:m.add_3dface(points,dxfattribs={'layer':'Faces'})
  else:
   points=[points[0],points[1],points[3],points[2]]
   getattr(m,'add_'+kind)(points,dxfattribs={'layer':'Faces'})
 if kind=='face-block':d.modelspace().add_blockref('FacesBlock',(0,0,0))
 m.add_line((8,0,0),(9,0,0),dxfattribs={'layer':'Faces'});m.add_text('ignored source annotation',dxfattribs={'layer':'Faces'});d.saveas(inputs/(kind+'.dxf'))
(inputs/'invalid.obj').write_text('v 0 0 0\nv not-a-number 1 0\nf 1 2 3\n');(inputs/'invalid.dxf').write_bytes(b'not a DXF document')
roster=[('obj-root','obj','color-root.obj','root','accept'),('obj-texture-leaf','obj','windows-options-leaf.obj','leaf','accept'),('dxf-block-leaf','dxf','shared-leaf.dxf','leaf','accept')]+[('dxf-'+k,'dxf',k+'.dxf','root','accept')for k in ['3dface','solid','trace','face-block']]+[('obj-unit-cancel','obj','color-root.obj','root','unit-cancel'),('dxf-unit-cancel','dxf','3dface.dxf','root','unit-cancel'),('obj-chooser-cancel','obj','color-root.obj','root','chooser-cancel'),('dxf-chooser-cancel','dxf','3dface.dxf','root','chooser-cancel'),('obj-invalid','obj','invalid.obj','root','failure'),('dxf-invalid','dxf','invalid.dxf','root','failure')]
errors=[];windows=[];timers=[];states=[];cases=[];dialogs=[]
def exception(t,e,tb):
 row={'type':t.__name__,'message':str(e),'traceback':''.join(traceback.format_exception(t,e,tb))};errors.append(row);print(row['traceback'],file=sys.stderr);(root/'callback-errors.json').write_text(json.dumps(errors,indent=2))
sys.excepthook=exception
def camera(w):
 c=w.viewport.camera;return {'target':list(c.target.toTuple()),'distance':c.distance,'yaw':c.yaw,'pitch':c.pitch,'projection':'perspective' if c.perspective else 'parallel','fov':c.fov_deg}
def record(w,name,phase):
 states.append({'case':name,'phase':phase,'scene':scene_state(w.viewport.scene),'camera':camera(w),'depth':len(w.viewport.scene._edit_stack),'undo':len(w.viewport.history.undo_stack),'redo':len(w.viewport.history.redo_stack),'preferences':{key:QSettings().value('import/'+key+'_unit')for key in ['obj','dxf']}})
 (root/'original-unit-import.partial.json').write_text(json.dumps({'cases':cases,'states':states,'callback_errors':errors},indent=2))
def modal(mode,name):
 done=[False];timed_out=[False];timer=QTimer();deadline=QTimer();deadline.setSingleShot(True);timers.extend([timer,deadline])
 def poll():
  d=app.activeModalWidget()
  if isinstance(d,QMessageBox):
   dialogs.append({'case':name,'kind':'error','text':d.text()});d.accept()
   if mode=='failure':done[0]=True;timer.stop();deadline.stop()
  elif isinstance(d,QInputDialog):
   try:
    if mode=='unit-cancel' or timed_out[0]:d.reject()
    else:
     combo=d.findChild(QComboBox);assert combo is not None;dialogs.append({'case':name,'kind':'unit','suggested_index':combo.currentIndex()});combo.setCurrentIndex(0 if name.startswith('obj') else 2)
     QTest.mouseClick(d.findChild(QDialogButtonBox).button(QDialogButtonBox.Ok),Qt.LeftButton)
    done[0]=True;timer.stop();deadline.stop()
   except BaseException:exception(*sys.exc_info());d.reject()
 def timeout():
  if done[0] or timed_out[0]:return
  timed_out[0]=True
  errors.append({'type':'FixtureTimeout','message':name,'traceback':''});d=app.activeModalWidget()
  if d is not None:d.reject()
 timer.timeout.connect(poll);deadline.timeout.connect(timeout);timer.start(10);deadline.start(5000)
 return timer,deadline
picker=main.file_dialogs.getOpenFileName
for name,kind,file,context,mode in roster:
 w=MainWindow();windows.append(w);w.show();app.processEvents();seed(w.viewport.scene)
 c=w.viewport.camera;c.target=V(0,0,0);c.distance=20.0;c.yaw=-0.7853981633974483;c.pitch=0.5235987755982988;c.perspective=False;c.fov_deg=45.0
 igz.save_scene(w.viewport.scene,root/(name+'-input.igz'))
 if context=='leaf':w.viewport.begin_group_edit(w.viewport.scene.groups[0]);w.viewport.begin_group_edit(w.viewport.scene.edit_group.children[0])
 record(w,name,'before');before=states[-1];owned=None;dialog_start=len(dialogs)
 main.file_dialogs.getOpenFileName=lambda *a,**k:('','')if mode=='chooser-cancel'else(str(inputs/file),kind)
 if mode!='chooser-cancel':owned=modal(mode,name)
 getattr(w,'_on_import_'+kind)();app.processEvents()
 if owned:
  for t in owned:t.stop()
 record(w,name,'import');saved=None
 if mode=='accept':
  assert states[-1]['scene']!=before['scene'] and states[-1]['undo']==before['undo']+1,'actual import missing'
  if kind=='obj':assert states[-1]['camera']==before['camera'],'OBJ must not fit camera'
  else:assert states[-1]['camera']!=before['camera'],'DXF actual fit missing'
  w.viewport.history.undo();record(w,name,'undo');w.viewport.history.redo();record(w,name,'redo')
  while w.viewport.scene._edit_stack:w.viewport.end_group_edit()
  record(w,name,'finish');saved=name+'-saved.igz';igz.save_scene(w.viewport.scene,root/saved)
 else:
  assert states[-1]['scene']==before['scene'] and (states[-1]['undo'],states[-1]['redo'])==(before['undo'],before['redo']),'refusal/cancel mutated document/history'
  assert states[-1]['camera']==before['camera']
 if mode=='failure':assert any(d['kind']=='error'for d in dialogs[dialog_start:]) or w.viewport.history.last_error,'actual failure witness missing'
 cases.append({'name':name,'kind':kind,'file':'assets/'+file,'input':name+'-input.igz','context':context,'mode':mode,'unit':'m','saved':saved,'initial_camera':before['camera'],'phases':[v['phase']for v in states if v['case']==name]})
 (root/'completed-cases.json').write_text(json.dumps({'cases':cases,'states':states},indent=2));w.hide()
main.file_dialogs.getOpenFileName=picker;assert not errors,errors
(root/'original-unit-import.json').write_text(json.dumps({'source_pin':PIN,'cases':cases,'states':states,'reused':reused,'dialogs':dialogs,'callback_errors':errors,'scope':'Only missing actual App ingress and DXF face cleanup; no recapture of accepted geometry/material/curve/block corpus.'},indent=2)+'\n');print(json.dumps({'cases':len(cases),'states':len(states)}))
