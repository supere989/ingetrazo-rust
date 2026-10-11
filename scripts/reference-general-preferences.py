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
QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,str(root/'source-settings'));app.setOrganizationName('PreferencesProof');app.setApplicationName('PreferencesProof')
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
w=W()
class Command:
 def do(self,scene):pass
for i in range(3):w.viewport.history.execute(Command())
rows=[]
def record(phase):
 st=QSettings();rows.append(dict(phase=phase,autosave=str(st.value(keys['autosave']))!='0',minutes=int(st.value(keys['minutes'])),undo_cap=int(st.value(keys['undo_cap'])),look=int(st.value(keys['look'])),wheel=str(st.value(keys['wheel']))!='0',orbit=str(st.value(keys['orbit']))!='0',section=str(st.value(keys['section']))!='0',opaque=st.value('proof/opaque'),undo=len(w.viewport.history.undo_stack),arms=w.arms))
def configure(d,cap):
 d._autosave.setChecked(False);d._autosave_min.setValue(7);d._undo_steps.setValue(cap);d._look_sens.setValue(47);d._invert.setChecked(True);d._invert_orbit.setChecked(True);d._ask_section.setChecked(False)
def press(d,button):d.findChild(QDialogButtonBox).button(button).click();app.processEvents()
record('before');d=PreferencesDialog(w);configure(d,0);record('draft');press(d,QDialogButtonBox.Cancel);record('cancel')
d=PreferencesDialog(w);configure(d,0);press(d,QDialogButtonBox.Ok);record('accept')
for i in range(2):w.viewport.history.execute(Command())
record('unlimited');d=PreferencesDialog(w);d._undo_steps.setValue(2);press(d,QDialogButtonBox.Ok);record('lower-no-prune');w.viewport.history.execute(Command());record('next-command-pruned')
w=W();d=PreferencesDialog(w);assert w.viewport.history.max_steps==2 and d._undo_steps.value()==2 and d._autosave_min.value()==7 and d._invert.isChecked();d.show();configure(d,0);QTest.keyClick(d,Qt.Key_Escape);app.processEvents();assert d.result()==0;record('restart-cancel')
(root/'original-preferences.json').write_text(json.dumps(dict(rows=rows),indent=2));print('actual PreferencesDialog OK/Cancel/Escape and History complete')
