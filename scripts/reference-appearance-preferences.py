#!/usr/bin/env python3
"""New source appearance observations; OS scheme events are simulated explicitly."""
import os,sys,json,re,hashlib,ast
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
repo=Path(__file__).resolve().parents[1];sys.path.insert(0,str((repo/'ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow,QToolBar,QDialogButtonBox
from PySide6.QtCore import QSettings,Qt
from PySide6.QtGui import QPalette,QImage
from core.scene import Scene
from core.history import History
from views.preferences_dialog import PreferencesDialog
from views import theme,icons
app=QApplication([]);root=Path(sys.argv[1]).resolve();root.mkdir(exist_ok=True,parents=True)
QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,str(root/'settings'));app.setOrganizationName('AppearanceProof');app.setApplicationName('AppearanceProof')
for k,v in {'general/autosave':'1','general/autosave_min':5,'general/undo_steps':200,'walk/look_sensitivity':25,'nav/invert_wheel':'0','nav/invert_orbit_y':'0','section/ask_name':'1'}.items():QSettings().setValue(k,v)
class VP:
 def __init__(self):self.scene=Scene();self.history=History(self.scene);self.edit_rest_mode='normal';self._msaa=4;self._invert_wheel=False;self._invert_orbit_y=False
 def update(self):pass
 def set_edit_rest_mode(self,m):self.edit_rest_mode=m
class W(QMainWindow):
 def __init__(self):super().__init__();self.viewport=VP();self.viewport.history.max_steps=200;self.tb=QToolBar(self);self.addToolBar(self.tb)
 def _setup_autosave(self):pass
# Execute the literal source method on a real QMainWindow/QToolBar, without constructing unrelated panels.
src=(repo/'ingetrazo/views/main_window.py').read_text();node=next(n for c in ast.parse(src).body if isinstance(c,ast.ClassDef) and c.name=='MainWindow' for n in c.body if isinstance(n,ast.FunctionDef) and n.name=='set_toolbar_icon_size');scope={'QToolBar':QToolBar};exec(compile(ast.Module(body=[node],type_ignores=[]),'source:set_toolbar_icon_size','exec'),scope);W.set_toolbar_icon_size=scope['set_toolbar_icon_size']
scheme=Qt.ColorScheme.Light;theme._system_scheme=lambda _app:scheme
w=W();rows=[]
def record(phase):
 p=app.palette();rows.append({'phase':phase,'theme':theme.saved_theme(),'dark':p.color(QPalette.Window).lightness()<128,'icon_px':icons.toolbar_icon_px(),'window':p.color(QPalette.Window).getRgb()[:3],'text':p.color(QPalette.WindowText).getRgb()[:3],'base':p.color(QPalette.Base).getRgb()[:3],'highlight':p.color(QPalette.Highlight).getRgb()[:3],'link':p.color(QPalette.Link).getRgb()[:3],'settings':{k:QSettings().value(k) for k in ('general/theme','ui/toolbar_icon_px','ui/large_toolbar_icons') if QSettings().contains(k)}})
def choose(t,px):
 d=PreferencesDialog(w);d._theme.setCurrentIndex(d._theme.findData(t));d._icon_px.setCurrentIndex(d._icon_px.findData(px));return d
def press(d,b):d.findChild(QDialogButtonBox).button(b).click();app.processEvents()
theme.apply_theme(app);record('default-dark')
d=choose('light',40);record('draft-unpublished');press(d,QDialogButtonBox.Cancel);record('cancel')
d=choose('light',40);press(d,QDialogButtonBox.Ok);assert w.tb.iconSize().width()==40;record('light-40')
d=choose('system',20);press(d,QDialogButtonBox.Ok);record('system-light')
scheme=Qt.ColorScheme.Dark;theme._follower.on_scheme(scheme);record('system-dark')
scheme=Qt.ColorScheme.Unknown;theme._follower.on_scheme(scheme);record('system-unknown')
d=choose('dark',24);press(d,QDialogButtonBox.Ok);record('fixed-dark')
QSettings().remove('ui/toolbar_icon_px');QSettings().setValue('ui/large_toolbar_icons','1');record('legacy-large')
d=choose('dark',40);press(d,QDialogButtonBox.Ok);assert not QSettings().contains('ui/large_toolbar_icons');record('migrated-40')
QSettings().setValue('general/theme','invalid');QSettings().setValue('ui/toolbar_icon_px','invalid');theme.apply_theme(app);record('invalid-fallback')
(root/'original-appearance.json').write_text(json.dumps({'rows':rows},indent=2))
# Freeze exact pinned QPainter output, in both source palettes, for all compiled icons.
names=sorted(set(re.findall(r'"([a-z0-9_]+)"\s*=> Some\(include_bytes!',(repo/'crates/ingetrazo-gui/src/icons.rs').read_text())))
assets={}
for mode in ('dark','light'):
 theme.apply_theme(app,mode);folder=root/'icons'/mode;folder.mkdir(parents=True,exist_ok=True)
 for name in names:
  icon=icons.tool_icon(name);assert not icon.isNull(),name
  im=icon.pixmap(48,48).toImage().convertToFormat(QImage.Format_RGBA8888);assert im.save(str(folder/(name+'.png')))
  pixels=bytes(im.constBits());assets[mode+'/'+name]={'size':[im.width(),im.height()],'rgba_sha256':hashlib.sha256(pixels).hexdigest(),'png_sha256':hashlib.sha256((folder/(name+'.png')).read_bytes()).hexdigest()}
(root/'source-icons.json').write_text(json.dumps(assets,indent=2))
print(json.dumps({'states':len(rows),'icons':len(assets),'system_events':'simulated via actual source follower','toolbar_callback':'literal source method on real QToolBar'}))
