#!/usr/bin/env python3
"""Actual frozen InlineTextEditor keyboard handler/document undo grouping."""
import os,sys,json,subprocess
from pathlib import Path
from types import SimpleNamespace,MethodType
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication,QGraphicsScene,QStatusBar
from PySide6.QtGui import QKeyEvent,QTextCursor
from PySide6.QtCore import Qt,QEvent
from core.composition import TextoItem,ComposerHistory
from views.composer import ComposerWindow,TextItem
pin='6be29fe437a7cd992d4e079f71720821b25216b9';assert subprocess.check_output(['git','rev-parse','HEAD'],cwd='/work/ingetrazo',text=True).strip()==pin
app=QApplication([]);status=QStatusBar();rows=[]
recipes={'contiguous':['alpha beta'],'newline':['hello','Return','world'],'replacement':['Control+a','replacement'],'cursor-boundary':['abcd','Left','Left','X'],'backspace':['abcd','Backspace','Backspace'],'two-newlines':['a','Return','b','Return','c'],'redo-backspace':['abcd','Undo','Redo','Backspace'],'redo-insert':['abcd','Undo','Redo','X'],'redo-cursor-insert':['abcd','Undo','Redo','Left','X'],'redo-newline':['abcd','Undo','Redo','Return'],'cursor-insert-delete':['abcd','Left','Left','X','Backspace','Backspace'],'cursor-insert-redo-delete':['abcd','Left','Left','X','Undo','Redo','Backspace','Backspace'],'single-insert-delete':['X','Backspace'],'multi-insert-delete-all':['abc','Backspace','Backspace','Backspace'],'repeated-fragment':['aaaa','Left','Left','a','Backspace','Backspace']}
for name,recipe in recipes.items():
 canvas=QGraphicsScene();owner=SimpleNamespace(canvas=canvas,history=ComposerHistory(),_mark_dirty=lambda:None,on_selection_changed=lambda:None,statusBar=lambda:status);owner.end_inline_edit=MethodType(ComposerWindow.end_inline_edit,owner);item=TextItem(owner,TextoItem(text='Text'));canvas.addItem(item);ComposerWindow.begin_inline_edit(owner,item);editor=owner._inline_editor
 def send(k,text='',mods=Qt.NoModifier):editor.keyPressEvent(QKeyEvent(QEvent.KeyPress,k,mods,text))
 for op in recipe:
  if op=='Undo':send(Qt.Key_Z,'z',Qt.ControlModifier)
  elif op=='Redo':send(Qt.Key_Z,'z',Qt.ControlModifier|Qt.ShiftModifier)
  elif op=='Control+a':send(Qt.Key_A,'a',Qt.ControlModifier)
  elif op in ('Return','Left','Right','Backspace','Delete','Home','End'):send(getattr(Qt,'Key_'+op),'\n' if op=='Return' else '')
  else:
   for char in op:send(Qt.Key_unknown,char)
 final=editor.toPlainText();undo=[]
 while editor.document().isUndoAvailable():send(Qt.Key_Z,'z',Qt.ControlModifier);undo.append(editor.toPlainText());assert len(undo)<30
 redo=[]
 while editor.document().isRedoAvailable():send(Qt.Key_Z,'z',Qt.ControlModifier|Qt.ShiftModifier);redo.append(editor.toPlainText());assert len(redo)<30
 rows.append(dict(name=name,recipe=recipe,final=final,undo=undo,redo=redo));editor.finish(False)
Path(sys.argv[1]).write_text(json.dumps(dict(source_pin=pin,scope='Actual original InlineTextEditor keyPressEvent and Qt document, synthetic direct-key control; genuine fullwindow simple recipe retained separately',rows=rows),indent=2)+'\n')
