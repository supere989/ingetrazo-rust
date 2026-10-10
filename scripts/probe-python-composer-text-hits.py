#!/usr/bin/env python3
"""Frozen actual QGraphicsScene Text/frame hit delivery and active lock commands."""
import os,sys,json,subprocess
from pathlib import Path
from types import SimpleNamespace,MethodType
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication,QGraphicsScene,QStatusBar
from PySide6.QtCore import QPointF,Qt
from core.composition import Composicion,ComposerHistory,TextoItem,MarcoVista,FormaItem
from views.composer import ComposerWindow,TextItem,FrameItem,FormaCanvasItem
pin='6be29fe437a7cd992d4e079f71720821b25216b9';assert subprocess.check_output(['git','rev-parse','HEAD'],cwd='/work/ingetrazo',text=True).strip()==pin
app=QApplication([]);status=QStatusBar();canvas=QGraphicsScene();owner=SimpleNamespace(canvas=canvas,comp=Composicion(),history=ComposerHistory(),_mark_dirty=lambda:None,on_selection_changed=lambda:None,statusBar=lambda:status)
owner.end_inline_edit=MethodType(ComposerWindow.end_inline_edit,owner)
model=TextoItem(x_mm=20,y_mm=30,w_mm=80,text='many words '*40,size_pt=14);text=TextItem(owner,model);canvas.addItem(text)
def rect(it):r=it.sceneBoundingRect();return [r.x(),r.y(),r.width(),r.height()]
def hit(x,y):return next((type(i).__name__ for i in canvas.items(QPointF(x,y)) if i.acceptedMouseButtons()&Qt.LeftButton),None)
rows=[dict(name='wrapped-authored-single-line',rect=rect(text),hits=[hit(19.6,30),hit(102.9,30),hit(103.1,30)])]
model.bg_color='#ffffff';rows.append(dict(name='background-hit-extension',rect=rect(text),hits=[hit(18.6,30),hit(18.4,30)]))
frame_model=MarcoVista(x_mm=20,y_mm=30,w_mm=80,h_mm=20,z=2);frame=FrameItem(owner,frame_model);canvas.addItem(frame);rows.append(dict(name='higher-frame',hit=hit(25,35)))
canvas.removeItem(frame);frame_model.locked=True;frame=FrameItem(owner,frame_model);canvas.addItem(frame);rows.append(dict(name='locked-higher-frame',hit=hit(25,35)))
canvas.removeItem(frame);shape=FormaCanvasItem(owner,FormaItem(x_mm=20,y_mm=30,w_mm=80,h_mm=20,z=3,kind='ellipse'));canvas.addItem(shape);rows.append(dict(name='unfilled-ellipse-bounding-box-hit',hit=hit(20,30)));canvas.removeItem(shape);ComposerWindow.begin_inline_edit(owner,text);owner._inline_editor.setPlainText('locked draft');model.locked=True;owner._inline_editor.finish(True);rows.append(dict(name='lock-during-draft-commit',text=model.text,locked=model.locked,undo=len(owner.history._undo)));owner.history.undo();rows.append(dict(name='undo-text-preserves-lock',text=model.text,locked=model.locked))
Path(sys.argv[1]).write_text(json.dumps(dict(source_pin=pin,rows=rows),indent=2)+'\n')
