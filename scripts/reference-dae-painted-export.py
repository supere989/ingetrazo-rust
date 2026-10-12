#!/usr/bin/env python3
"""Actual unchanged MainWindow DAE export; finite declared fixture publication."""
import os,sys,json,hashlib,subprocess,traceback,shutil,time
from pathlib import Path
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
repo=Path(__file__).resolve().parents[1];root=Path(sys.argv[1]).resolve();root.mkdir(parents=True,exist_ok=True)
assert not (root/'original-dae-export.json').exists()
contract=json.loads((repo/'docs/audits/dae-painted-export/fixture-contract.json').read_text())
original=(repo/'ingetrazo').resolve()
for row in contract['original_sources']:assert hashlib.sha256((original/row['path']).read_bytes()).hexdigest()==row['sha256']
assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()==PIN
for kind in ['CONFIG','DATA','CACHE']:os.environ['XDG_'+kind+'_HOME']=str(root/('xdg-'+kind.lower()))
os.environ['QT_QPA_PLATFORM']='offscreen';sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication,QMessageBox
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from PySide6.QtCore import QSettings,qVersion
from views.main_window import MainWindow
import views.main_window as main
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.materials import Material
from core.layers import Layer
from formats import igz
from georef.datum import SceneDatum
import exchange_import_observations as observe
app=QApplication([]);assert qVersion()=='6.12.0';app.setOrganizationName('DaeExportProof');app.setApplicationName('DaeExportProof')
QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,str(root/'settings'))
assets=root/'assets';assets.mkdir();parent=repo/'docs/audits/gltf-rich';provenance=json.loads((parent/'provenance.json').read_text());reused=[]
for name in ['leaves.png','opaque.jpg']:
 path=parent/'original'/name;b=path.read_bytes();assert hashlib.sha256(b).hexdigest()==provenance['artifacts_sha256']['original/'+name];(assets/name).write_bytes(b);reused.append({'path':str(path.relative_to(repo)),'sha256':hashlib.sha256(b).hexdigest(),'size':len(b)})
for dirname,name in [('a','leaves.png'),('b','opaque.jpg')]:
 (assets/dirname).mkdir();(assets/dirname/'shared.png').write_bytes((assets/name).read_bytes())
def face(mesh,x=0,attrs=None,holes=None):
 f=mesh.add_face([V(x,0,0),V(x+3,0,0),V(x+3,2,0),V(x,2,0)],holes);f.attrs.update(attrs or {});f.attrs['vendor_dae_face']={'keep':[1,'opaque',False]};return f
recipes=[]
s=Scene();face(s.mesh);face(s.mesh,4,{'color':(.2,.4,.6),'back_color':(1.,0.,1.),'opacity':.55,'mat':'1 copper'});face(s.mesh,8,{'color':(.2,.4,.6),'opacity':.35,'mat':'1 copper'});f=s.mesh.add_face([V(0,4,0),V(5,4,0),V(5,9,0),V(0,9,0)],[[V(1,5,0),V(1,7,0),V(3,7,0),V(3,5,0)]]);f.attrs={'color':(.7,.3,.2)};recipes.append(('colors-default-hole',s))
s=Scene();face(s.mesh,attrs={'texture':{'path':str(assets/'leaves.png'),'sw':2.,'sh':3.,'rot':30.},'mat':'leaf material'});face(s.mesh,4,{'texture':{'path':str(assets/'leaves.png'),'uvw':[.5,0,0,.25,0,.5,0,.125]},'mat':'leaf material'});recipes.append(('textures-affine-planar',s))
s=Scene();mesh=Mesh();face(mesh,attrs={'color':(.25,.5,.75),'texture':{'path':str(assets/'opaque.jpg'),'uvw':[.5,0,0,0,0,.5,0,0]}})
for x in [2,8]:
 g=Group(mesh,'shared leaf');g.xform=M();g.xform.translate(x,3,4);g.xform.scale(-2,3,.5);parentg=Group(Mesh(),'parent');parentg.xform=M();parentg.xform.rotate(90,0,0,1);parentg.children=[g];s.groups.append(parentg)
g=Group(Mesh(),'hidden root');face(g.mesh,20);g.hidden=True;s.groups.append(g)
g=Group(Mesh(),'figure');face(g.mesh,24);g.billboard=True;s.groups.append(g)
g=Group(Mesh(),'hidden child');face(g.mesh,28,{'hidden':True,'color':(.7,.4,.2)});g.hidden=True;parentg=Group(Mesh(),'visible parent');parentg.children=[g];s.groups.append(parentg);recipes.append(('nested-shared-reflected',s))
s=Scene();s.georef=SceneDatum(-16.4,-71.5,2340,12)
for i,(color,path) in enumerate([((.2,.3,.4),assets/'a/shared.png'),((.6,.2,.1),assets/'b/shared.png')]):
 face(s.mesh,4*i,{'texture':{'path':str(path),'sw':1.,'sh':1.},'mat':'Mármol & stone'})
face(s.mesh,8,{'color':(.2,.3,.4),'mat':'same name'});face(s.mesh,12,{'color':(.6,.2,.1),'mat':'same name'});recipes.append(('datum-names-collisions',s))
s=Scene();face(s.mesh,attrs={'color':(.1,.2,.3)});recipes.append(('chooser-cancel-suffix',s))
s=Scene();face(s.mesh,attrs={'texture':{'path':str(assets/'absent.png'),'sw':1.,'sh':1.}});recipes.append(('missing-resource-write-error',s))
def texture(t):
 t=dict(t);path=Path(t.pop('path'))
 t['resource']={'encoded_sha256':hashlib.sha256(path.read_bytes()).hexdigest()} if path.is_file() else {'encoded_sha256':None,'missing':True}
 return t
observe.texture=texture
cases=[];states=[];errors=[];dialogs=[];time_bounds=[];exports=root/'exports';exports.mkdir();active={'case':None,'phase':'initial'}
def error(typ,e,tb):
 row={**active,'type':typ.__name__,'message':str(e),'traceback':''.join(traceback.format_exception(typ,e,tb))};errors.append(row);print(row['traceback'],file=sys.stderr)
sys.excepthook=error
old_picker=main.file_dialogs.getSaveFileName;old_critical=QMessageBox.critical
QMessageBox.critical=lambda *args:dialogs.append({**active,'error':str(args[2])})
def camera_state(camera):
 d=dict(vars(camera));d['target']=list(camera.target.toTuple());d['up']=list(camera.up.toTuple());return d
def record(host,name,phase,outcome):
 c=host.viewport.camera
 states.append({'case':name,'phase':phase,'scene':observe.scene_state(host.viewport.scene),'current_file':host._current_path.name,'dirty':host._is_dirty(),'camera':{'target':list(c.target.toTuple()),'distance':c.distance,'yaw':c.yaw,'pitch':c.pitch,'projection':'perspective' if c.perspective else 'parallel','fov':c.fov_deg},'edit_context':len(getattr(host.viewport.scene,'_edit_stack',[])),'undo':len(host.viewport.history.undo_stack),'redo':len(host.viewport.history.redo_stack),'composer_history':None,'outcome':outcome})
def flush():
 report={'source_pin':PIN,'qt_version':qVersion(),'cases':cases,'states':states,'callback_errors':errors,'dialogs':dialogs,'reused':reused,'export_time_bounds':time_bounds,'scope':'Actual controlled save-chooser callback and source export; no physical File-menu click or viewport-pixel claim'}
 (root/'original-dae-export.partial.json').write_text(json.dumps(report,indent=2)+'\n');return report
try:
 for name,s in recipes:
  active.update(case=name,phase='input');inputfile=name+'-input.igz';igz.save_scene(s,root/inputfile)
  host=MainWindow();igz.load_into(host.viewport.scene,root/inputfile);host._current_path=root/'fixture-project.igz';host._saved_version=host.viewport.scene.version-1
  case={'name':name,'input':inputfile,'phases':[],'exports':[],'saved':name+'-saved.igz','initial_file':'fixture-project.igz','initial_dirty':True};cases.append(case)
  def phase(label,chosen=None):
   active['phase']=label;before=observe.scene_state(host.viewport.scene);epoch=host.viewport.scene.version;history=(len(host.viewport.history.undo_stack),len(host.viewport.history.redo_stack));current=host._current_path;dirty=host._is_dirty();cam=dict(vars(host.viewport.camera));cam['target']=list(host.viewport.camera.target.toTuple());cam['up']=list(host.viewport.camera.up.toTuple());n=len(dialogs)
   if label=='before':outcome='ok'
   else:
    main.file_dialogs.getSaveFileName=lambda *args:(str(chosen) if chosen else '', '')
    started=time.time();host._on_export_dae();ended=time.time();time_bounds.append({'case':name,'phase':label,'started':started,'ended':ended});outcome='cancel' if chosen is None else ('failed' if len(dialogs)>n else 'ok')
   assert observe.scene_state(host.viewport.scene)==before and host.viewport.scene.version==epoch and (len(host.viewport.history.undo_stack),len(host.viewport.history.redo_stack))==history and host._current_path==current and host._is_dirty()==dirty and camera_state(host.viewport.camera)==cam
   case['phases'].append(label);record(host,name,label,outcome)
   if chosen is not None and outcome=='ok':case['exports'].append({'phase':label,'file':str(chosen.with_suffix('.dae').relative_to(root))})
   flush()
  phase('before')
  if name=='chooser-cancel-suffix':
   phase('cancel');p=exports/(name+'-coerced.wrong');p.with_suffix('.dae').write_bytes(b'prior destination');phase('suffix',p)
  elif name=='missing-resource-write-error':
   phase('missing',exports/(name+'-export.dae'));p=exports/(name+'-blocked.dae');p.mkdir();phase('write-error',p);assert states[-1]['outcome']=='failed'
  else:phase('export',exports/(name+'-export.dae'))
  igz.save_scene(host.viewport.scene,root/case['saved']);active['phase']='teardown';host.hide();host.deleteLater();app.processEvents()
 assert not errors
 report=flush();(root/'original-dae-export.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'cases':len(cases),'states':len(states),'exports':sum(len(c['exports'])for c in cases)}))
finally:main.file_dialogs.getSaveFileName=old_picker;QMessageBox.critical=old_critical
