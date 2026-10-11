#!/usr/bin/env python3
"""Pinned literal MainWindow recent methods, actual QMenu and isolated QSettings."""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMenu,QMessageBox
from PySide6.QtCore import QSettings
from views.main_window import MainWindow
app=QApplication([]);root=Path(sys.argv[1]).resolve();root.mkdir(parents=True,exist_ok=True)
QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,str(root/'qt-settings'))
app.setOrganizationName('RecentProof');app.setApplicationName('RecentProof')
class Host:
 _RECENT_MAX=10
 _recent_paths=staticmethod(MainWindow._recent_paths)
 _remember_recent=MainWindow._remember_recent
 _fill_recent_menu=MainWindow._fill_recent_menu
 _open_recent=MainWindow._open_recent
 def __init__(self):
  self._recent_menu=QMenu();self.events=[];self.allow=False;self.viewport=self
 def end_group_edit(self):self.events.append('finish')
 def _confirm_discard(self,*args):self.events.append('prompt');return self.allow
 def open_path(self,p):self.events.append('open:'+p.name)
h=Host();QSettings().setValue('recent_files',[])
paths=[]
for i in range(12):
 p=root/f'doc-{i}.igz';p.write_text('{}');paths.append(p);h._remember_recent(p)
rows=[]
def record(phase):rows.append(dict(phase=phase,recent=[Path(p).name for p in h._recent_paths()]))
record('cap');h._remember_recent(root/'./doc-5.igz');record('alias');h._remember_recent(root/'model.obj');record('import');h._remember_recent(None);record('none')
h._fill_recent_menu();menu=[dict(label=a.text(),tooltip=a.toolTip(),enabled=a.isEnabled(),separator=a.isSeparator())for a in h._recent_menu.actions()]
paths[5].unlink();h._fill_recent_menu();filtered=[a.toolTip()for a in h._recent_menu.actions()if a.toolTip()and not a.isSeparator()and a.text()!='Clear list']
record('missing-filter-retains')
old=QMessageBox.warning;QMessageBox.warning=lambda *a:h.events.append('warning')
h._open_recent(paths[5]);missing=list(h.events);h.events=[]
h._open_recent(paths[11]);cancel=list(h.events);h.events=[];h.allow=True;h._open_recent(paths[11]);success=list(h.events)
QMessageBox.warning=old
# Invoke actual QAction's persisted Clear callback.
h._recent_menu.actions()[-1].trigger();record('clear');QSettings().sync();restart=MainWindow._recent_paths()
h._fill_recent_menu();empty=[dict(label=a.text(),enabled=a.isEnabled())for a in h._recent_menu.actions()]
(root/'original-recent.json').write_text(json.dumps(dict(rows=rows,menu=menu,filtered=[Path(p).name for p in filtered],missing=missing,cancel=cancel,success=success,restart=restart,empty=empty),indent=2))
print('literal source/Qt menu complete')
