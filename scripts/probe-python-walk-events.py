#!/usr/bin/env python3
import sys,json,tempfile
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ingetrazo'))
from PySide6.QtCore import QPointF,Qt,QEvent,QSettings
from PySide6.QtGui import QVector3D,QMouseEvent
from PySide6.QtWidgets import QApplication
from views.main_window import MainWindow
app=QApplication([]);app.setOrganizationName('FirstPersonLookIndependent');app.setApplicationName('isolated');QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,tempfile.mkdtemp());QSettings().setValue("walk/look_sensitivity",25)
rows=[]
from tools.base import ToolContext
for name in ['split-press-motion','press-motion-one-frame','press-motion-release-one-frame','hover-control-released-before-tick']:
 w=MainWindow();vp=w.viewport;vp.resize(800,600);vp.camera.aspect=800/600;vp.camera.distance=10;vp.camera.look_from(QVector3D(0,0,1.68),QVector3D(0,1,0));w._activate_tool('walk');tool=vp.active_tool
 class E:
  def __init__(self,x,y,mods):self.p=QPointF(x,y);self.mods=mods
  def position(self):return self.p
  def modifiers(self):return self.mods
 mods=Qt.ControlModifier if name=='hover-control-released-before-tick'else Qt.NoModifier
 tool.on_click(vp._build_ctx(E(400,300,mods)));tool.on_hover(vp._build_ctx(E(400,240,mods)))
 if name=='press-motion-release-one-frame':tool.on_release(vp)
 tool._tick();rows.append(dict(name=name,eye=[vp.camera.eye().x(),vp.camera.eye().y(),vp.camera.eye().z()],forward=[vp.camera.forward().x(),vp.camera.forward().y(),vp.camera.forward().z()],undo=len(vp.history.undo_stack)));tool.on_cancel(vp);w.deleteLater()
Path(sys.argv[1]).write_text(json.dumps(dict(cases=rows,scope='actual frozen Walk contexts/on_click/on_hover and one real _tick slot invocation; no wall-clock/render claim'),indent=2)+'\n')
