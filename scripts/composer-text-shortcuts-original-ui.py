#!/usr/bin/env python3
"""Actual original MainWindow/ComposerWindow; constructors and observation only."""
import os,sys,json,time
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication,QToolButton
from PySide6.QtGui import QKeySequence
from PySide6.QtCore import QTimer,QPointF,QEvent,QObject
from views.main_window import MainWindow
from views.composer import ComposerWindow
from core.composition import Composicion
app=QApplication([]);win=MainWindow();sheet=Composicion();sheet.frames.clear();win.viewport.scene.compositions=[sheet];comp=ComposerWindow(win);comp.show();out=Path(os.environ['INGETRAZO_TEXT_AUTHORING_OUT']);events=[];keys=[]
class Observe(QObject):
 def eventFilter(self,obj,event):
  if event.type() in (QEvent.MouseButtonPress,QEvent.MouseButtonRelease,QEvent.MouseButtonDblClick):
   events.append({'at':time.monotonic(),'type':int(event.type()),'pos':[event.position().x(),event.position().y()]})
  if event.type()==QEvent.KeyPress:keys.append({"at":time.monotonic(),"key":int(event.key()),"modifiers":int(event.modifiers().value),"text":event.text()})
  return False
observer=Observe();comp._view.viewport().installEventFilter(observer);comp._view.installEventFilter(observer)
def point(scene):
 p=comp._view.viewport().mapToGlobal(comp._view.mapFromScene(QPointF(*scene)));p=comp.mapFromGlobal(p);return [p.x(),p.y()]
def snapshot():
 action=comp._tool_actions['texto'];button=next((b for b in comp.findChildren(QToolButton)if b.defaultAction()is action),None);p=comp.mapFromGlobal(button.mapToGlobal(button.rect().center()))if button else None;editor=getattr(comp,'_inline_editor',None);inline=editor.toPlainText()if editor else None;state={'key_bindings':{n:[s.toString()for s in QKeySequence.keyBindings(getattr(QKeySequence.StandardKey,n))]for n in ['Undo','Redo']},'key_events':keys[-32:],'window_id':int(comp.winId()),'serial':len(events),'pointer_events':events[-16:],'add_text':[p.x(),p.y()]if p else None,'paper_transform':[*point((0.,0.)),comp._view.transform().m11()],'inline':inline,'inline_rect':None,'text_tool':comp.tool_mode=='texto','history':[len(comp.history._undo),len(comp.history._redo)],'selected':None if not comp.canvas.selectedItems()else 0,'texts':[{'id':str(id(t)),'text':t.text,'pos':[t.x_mm,t.y_mm]}for t in comp.comp.texts]};(out/'state.tmp').write_text(json.dumps(state));(out/'state.tmp').replace(out/'state.json')
timer=QTimer();timer.timeout.connect(snapshot);timer.start(30);sys.exit(app.exec())
