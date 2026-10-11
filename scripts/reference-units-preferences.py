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
QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,str(root/'source-settings'));app.setOrganizationName('UnitsProof');app.setApplicationName('UnitsProof')
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
from core import units
from formats import igz
w=W();units.apply_units(w.viewport.scene,{'length':'ft-in','precision':4});w.viewport.scene.dimension_style['units']='mm';w.viewport.scene.dimension_style['decimals']=5;w.viewport.scene.dimension_style['vendor']={'opaque':[1,'x']}
w.viewport.scene.units['vendor']='replace-on-explicit-change'
igz.save_scene(w.viewport.scene,root/'input.igz')
rows=[]
def record(phase):
 s=w.viewport.scene;d=PreferencesDialog(w)
 rows.append(dict(phase=phase,units=units.model_units_of(s),style={k:s.dimension_style.get(k)for k in ['units','decimals','vendor']},default=units.new_document_units(),checkbox=d._units_for_new.isChecked(),undo=len(w.viewport.history.undo_stack)))
 d.reject()
def choose(code,n,new=False):
 d=PreferencesDialog(w);d._unit.setCurrentIndex(d._unit.findData(code));d._decimals.setValue(n);d._units_for_new.setChecked(new);return d
def press(d,button):d.findChild(QDialogButtonBox).button(button).click();app.processEvents()
record('loaded-composite');d=choose('m',0,True);press(d,QDialogButtonBox.Cancel);record('cancel')
# An unchanged unit pair must not overwrite deliberately independent dimension style.
d=choose('ft-in',4,False);press(d,QDialogButtonBox.Ok);record('unchanged')
for i,code in enumerate(units.UNIT_CHOICES):
 d=choose(code,i%7,i==7);press(d,QDialogButtonBox.Ok);record('apply-'+code);igz.save_scene(w.viewport.scene,root/(code+'.igz'))
w=W();units.apply_units(w.viewport.scene,units.new_document_units());record('restart-new')
igz.load_into(w.viewport.scene,root/'input.igz');record('open-retains-document')
# Unchecked OK leaves persisted defaults unchanged; a malformed default falls back.
d=choose('cm',6,False);press(d,QDialogButtonBox.Ok);record('unchecked-default')
QSettings().setValue(units.NEW_DOCUMENT_KEY,'{invalid');w=W();units.apply_units(w.viewport.scene,units.new_document_units());record('malformed-new')
(root/'original-units.json').write_text(json.dumps({'rows':rows},indent=2));print('actual source unit preferences and New/Open complete')
