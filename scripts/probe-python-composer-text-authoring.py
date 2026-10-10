#!/usr/bin/env python3
"""Actual frozen source placement, InlineTextEditor and ComposerHistory commands."""
import os,sys,json,subprocess
from pathlib import Path
from types import SimpleNamespace,MethodType
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication,QGraphicsScene,QStatusBar
from PySide6.QtGui import QAction
from core.composition import Composicion,ComposerHistory
from views.composer import ComposerWindow,TextItem
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd='/work/ingetrazo',text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
app=QApplication([]);comp=Composicion();comp.frames.clear();history=ComposerHistory();canvas=QGraphicsScene();status=QStatusBar();owner=SimpleNamespace(comp=comp,history=history,canvas=canvas,tool_mode='texto',_tool_actions={'select':QAction()},_mark_dirty=lambda:None,on_selection_changed=lambda:None,statusBar=lambda:status)
owner.STICKY_TOOLS=ComposerWindow.STICKY_TOOLS;owner._next_z=MethodType(ComposerWindow._next_z,owner);owner._after_place=MethodType(ComposerWindow._after_place,owner);owner.end_inline_edit=MethodType(ComposerWindow.end_inline_edit,owner)
rows=[]
def record(label):rows.append({'label':label,'texts':[dict(x=t.x_mm,y=t.y_mm,text=t.text,z=t.z)for t in comp.texts],'undo':len(history._undo),'redo':len(history._redo),'mode':owner.tool_mode})
ComposerWindow.place_tool(owner,23.,41.,80.,60.);record('placed');item=TextItem(owner,comp.texts[0]);canvas.addItem(item);ComposerWindow.begin_inline_edit(owner,item);owner._inline_editor.setPlainText('Hello\nworld\n\n');record('draft');owner._inline_editor.finish(True);record('committed');history.undo();record('undo');history.redo();record('redo');ComposerWindow.begin_inline_edit(owner,item);owner._inline_editor.setPlainText('discard');owner._inline_editor.finish(False);record('cancel');ComposerWindow.begin_inline_edit(owner,item);owner._inline_editor.finish(True);record('noop');item.model.locked=True;ComposerWindow.begin_inline_edit(owner,item);assert owner._inline_editor is None;record('locked')
Path(sys.argv[1]).write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','rows':rows,'scope':'Original operative placement and real InlineTextEditor finish on genuine QGraphicsScene; actual ComposerHistory, not pointer gestures'},indent=2)+'\n')
