#!/usr/bin/env python3
"""Actual original composer widgets, observed without replacing their handlers."""
import os,sys,json
from pathlib import Path
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow,QColorDialog,QLineEdit,QDialogButtonBox
from PySide6.QtCore import QTimer
from PySide6.QtGui import QShortcut,QKeySequence
from views.viewport import Viewport
from views.composer import ComposerWindow
from core.composition import Composicion,TextoItem
app=QApplication([]);host=QMainWindow();host.viewport=Viewport();host._current_path='/tmp/dynamic-fields.igz'
source=json.loads(Path(os.environ['INGETRAZO_PDF_UI_FIELD_OBSERVATIONS']).read_text())
host.viewport.scene.compositions=[Composicion(name=c['sheet_name'],landscape=False,texts=[TextoItem(text=c['authored_text'],size_pt=14)])for c in source['cases']]
window=ComposerWindow(host);shortcut_activations=[0]
def activated():shortcut_activations[0]+=1
for shortcut in window.findChildren(QShortcut):
 if shortcut.key().matches(QKeySequence(QKeySequence.StandardKey.Undo))==QKeySequence.SequenceMatch.ExactMatch:shortcut.activated.connect(activated)
window.resize(1400,950);window.show();out=Path(os.environ['INGETRAZO_ORIGINAL_FIELD_UI_OUT'])
def point(widget):
 p=widget.mapToGlobal(widget.rect().center());return [p.x(),p.y()]
def observe():
 c=window.comp;dialog=app.activeModalWidget();hexes=[];ok=None
 if isinstance(dialog,QColorDialog):
  hexes=[dict(name=w.objectName(),text=w.text(),point=point(w))for w in dialog.findChildren(QLineEdit)]
  buttons=dialog.findChildren(QDialogButtonBox)
  if buttons and buttons[0].button(QDialogButtonBox.StandardButton.Ok):ok=point(buttons[0].button(QDialogButtonBox.StandardButton.Ok))
 value={'source_undo_activations':shortcut_activations[0],'undo':len(window.history._undo),'redo':len(window.history._redo),'window_id':int(window.winId()),'toggle':point(window.border_check),'color_button':point(window.border_color_btn),'border':{'on':c.border,'mm':c.border_mm,'color':c.border_color,'radius_mm':c.border_radius_mm,'style':c.border_style},'dialog_hexes':hexes,'dialog_ok':ok,'dialog_id':int(dialog.winId())if isinstance(dialog,QColorDialog)else None,'authored_text':c.texts[0].text,'source_runtime_equal_second':c==host.viewport.scene.compositions[1]}
 (out/'state.tmp').write_text(json.dumps(value));(out/'state.tmp').replace(out/'state.json')
timer=QTimer();timer.timeout.connect(observe);timer.start(30);app.exec()
