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
for name in ['left-move-release','left-press-move-release','right-move-release','right-press-move-release','left-relative-move-release','right-relative-move-release','unrelated-release-retains-left','left-release-ends-with-right-held','unowned-relative-before-press']:
 w=MainWindow();vp=w.viewport;vp.resize(800,600);vp.camera.distance=10;vp.camera.look_from(QVector3D(0,0,1.68),QVector3D(0,1,0));vp.camera.perspective=True;w._activate_tool('first_person');held=Qt.NoButton
 def event(kind,button=Qt.NoButton,pos=QPointF(400,300)):
  global held
  if kind==QEvent.MouseButtonPress:held|=button
  if kind==QEvent.MouseButtonRelease:held&=~button
  e=QMouseEvent(kind,pos,pos,button,held,Qt.NoModifier)
  if kind==QEvent.MouseButtonPress:vp.mousePressEvent(e)
  elif kind==QEvent.MouseButtonRelease:vp.mouseReleaseEvent(e)
  else:vp.mouseMoveEvent(e)
 button=Qt.RightButton if name.startswith('right')else Qt.LeftButton
 before=vp.camera.eye()
 if name=='unowned-relative-before-press':event(QEvent.MouseMove,pos=QPointF(200,200))
 event(QEvent.MouseButtonPress,button)
 if name=='unrelated-release-retains-left':event(QEvent.MouseButtonRelease,Qt.RightButton)
 if name=='left-release-ends-with-right-held':event(QEvent.MouseButtonPress,Qt.RightButton);event(QEvent.MouseButtonRelease,Qt.LeftButton)
 event(QEvent.MouseMove,pos=QPointF(440,260));event(QEvent.MouseButtonRelease,button)
 if name=='left-release-ends-with-right-held':event(QEvent.MouseButtonRelease,Qt.RightButton)
 rows.append(dict(name=name,eye=[vp.camera.eye().x(),vp.camera.eye().y(),vp.camera.eye().z()],before_eye=[before.x(),before.y(),before.z()],yaw=vp.camera.yaw,pitch=vp.camera.pitch,captured=vp._look_drag is not None,undo=len(vp.history.undo_stack)));w.deleteLater();app.processEvents()
Path(sys.argv[1]).write_text(json.dumps(dict(cases=rows,scope='actual frozen Viewport mouse event handlers/FirstPerson.on_look, controlled ordered QMouseEvents; not rendered XTEST'),indent=2)+'\n')
