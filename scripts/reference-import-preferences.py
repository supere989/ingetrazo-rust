#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow,QDialogButtonBox
from PySide6.QtCore import QSettings,Qt
from PySide6.QtTest import QTest
from core.scene import Scene
from core.history import History
from views.preferences_dialog import PreferencesDialog
app=QApplication([]);root=Path(sys.argv[1]).resolve();root.mkdir(parents=True,exist_ok=True)
QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,str(root/'source-settings'));app.setOrganizationName('ImportPreferencesProof');app.setApplicationName('ImportPreferencesProof')
keys={'autosave':'general/autosave','minutes':'general/autosave_min','undo_cap':'general/undo_steps','look':'walk/look_sensitivity','wheel':'nav/invert_wheel','orbit':'nav/invert_orbit_y','section':'section/ask_name'}
seed={'autosave':'1','minutes':5,'undo_cap':200,'look':25,'wheel':'0','orbit':'0','section':'1'}
for k,v in seed.items():QSettings().setValue(keys[k],v)
QSettings().setValue('proof/opaque','vendor-preserved')
class VP:
 def __init__(self):self.scene=Scene();self.history=History(self.scene);self.edit_rest_mode='normal';self._msaa=4;self._invert_wheel=False;self._invert_orbit_y=False
 def update(self):pass
 def set_edit_rest_mode(self,m):self.edit_rest_mode=m
class W(QMainWindow):
 def __init__(self):super().__init__();self.viewport=VP();self.viewport.history.max_steps=int(QSettings().value('general/undo_steps',200));self.arms=0
 def _setup_autosave(self):self.arms+=1
import ast
from PySide6.QtCore import QTimer
from PySide6.QtWidgets import QInputDialog,QComboBox
from PySide6.QtGui import QVector3D,QMatrix4x4
from core.i18n import tr
from core.history import SnapshotImport
from formats import obj,dxf_in
# Execute the two unmodified literal MainWindow confirmation methods.
tree=ast.parse(Path('ingetrazo/views/main_window.py').read_text());cls=next(n for n in tree.body if isinstance(n,ast.ClassDef)and n.name=='MainWindow')
for name in ['_obj_unit','_dxf_unit']:
 node=next(n for n in cls.body if isinstance(n,ast.FunctionDef)and n.name==name);exec(compile(ast.fix_missing_locations(ast.Module(body=[node],type_ignores=[])),'pinned-main-window.py','exec'));setattr(W,name,globals()[name])
def dxf(length,header):
 return '0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n'+str(header)+'\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nTest\n10\n0\n20\n0\n30\n0\n11\n'+str(length)+'\n21\n0\n31\n0\n0\nENDSEC\n0\nEOF\n'
for name,span in [('ambiguous',1),('large',201),('huge',20001)]:
 (root/(name+'.obj')).write_text(f'v 0 0 0\nv {span} 0 0\nv 0 {span} 0\nf 1 2 3\n')
for name,length,code in [('header',1000,4),('lying',1000,6),('tiny',0.01,0)]: (root/(name+'.dxf')).write_text(dxf(length,code))
w=W();prefrows=[]
def pref(phase):
 prefrows.append({'phase':phase,'obj':str(QSettings().value('import/obj_unit','m')or'm'),'dxf':str(QSettings().value('import/dxf_unit','mm')or'mm')})
pref('default');dlg=PreferencesDialog(w);dlg._obj_unit.setCurrentIndex(dlg._obj_unit.findData('ft'));dlg._dxf_unit.setCurrentIndex(dlg._dxf_unit.findData('in'));pref('draft');dlg.reject();pref('cancel')
dlg=PreferencesDialog(w);dlg._obj_unit.setCurrentIndex(dlg._obj_unit.findData('ft'));dlg._dxf_unit.setCurrentIndex(dlg._dxf_unit.findData('in'));dlg.findChild(QDialogButtonBox).button(QDialogButtonBox.Ok).click();pref('accept');w=W();pref('restart')
rows=[]
cases=[('obj-cancel','ambiguous.obj',None),('obj-stored','ambiguous.obj','ft'),('obj-size','large.obj','cm'),('obj-huge','huge.obj','mm'),('obj-explicit','ambiguous.obj','in'),('dxf-header','header.dxf','mm'),('dxf-lying','lying.dxf','cm'),('dxf-stored','tiny.dxf','ft')]
def points(scene):
 all=[]
 def mesh(m,x):
  all.extend([list((x.map(v.position)if x else v.position).toTuple())for v in m.vertices])
 def groups(gs,parent=None):
  for g in gs:
   x=getattr(g,'xform',None)
   if parent is not None:x=parent*x if x is not None else parent
   mesh(g.mesh,x);groups(g.children,x)
 mesh(scene.mesh,None);groups(scene.groups)
 return sorted(all)
for case,file,chosen in cases:
 w=W();s=w.viewport.scene;s.mesh.add_edge(QVector3D(-1,0,0),QVector3D(0,0,0))
 initial=[]
 def answer():
  dlg=app.activeModalWidget();assert isinstance(dlg,QInputDialog)
  combo=dlg.findChild(QComboBox);keys=['m','cm','mm','in','ft']if file.endswith('.obj')else['mm','cm','m','in','ft'];initial.append(keys[combo.currentIndex()])
  if chosen is None:dlg.reject()
  else:combo.setCurrentIndex(keys.index(chosen));dlg.accept()
 QTimer.singleShot(0,answer)
 if file.endswith('.obj'):f=w._obj_unit(root/file)
 else:f=w._dxf_unit(root/file,dxf_in.suggest_unit_scale(dxf_in.open_document(root/file))[0])
 def record(phase):rows.append({'case':case,'phase':phase,'suggested':initial[0],'factor':f,'obj':str(QSettings().value('import/obj_unit','m')),'dxf':str(QSettings().value('import/dxf_unit','mm')),'points':points(s),'undo':len(w.viewport.history.undo_stack),'redo':len(w.viewport.history.redo_stack)})
 record('confirmed')
 if f is not None:
  if file.endswith('.obj'):loader=lambda scene:obj.load_obj(scene,root/file,scale=f)
  else:loader=lambda scene:dxf_in.load_dxf(scene,root/file,scale=f,name=Path(file).stem)
  w.viewport.history.execute(SnapshotImport(loader));record('import');w.viewport.history.undo();record('undo');w.viewport.history.redo();record('redo')
QSettings().sync();(root/'original-import-preferences.json').write_text(json.dumps({'preferences':prefrows,'rows':rows},indent=2));print('actual Qt unit confirmations and source SnapshotImport complete')
