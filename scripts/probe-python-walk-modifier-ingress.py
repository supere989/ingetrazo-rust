#!/usr/bin/env python3
import sys,json,tempfile
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ingetrazo'))
from PySide6.QtCore import QPointF,Qt,QEvent,QSettings
from PySide6.QtGui import QVector3D,QMouseEvent,QKeyEvent
from PySide6.QtWidgets import QApplication
from views.main_window import MainWindow
app=QApplication([]);app.setOrganizationName('FirstPersonLookIndependent');app.setApplicationName('isolated');QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,tempfile.mkdtemp());QSettings().setValue("walk/look_sensitivity",25)
rows=[]
for name in ['source-no-control','source-control-released-after-motion']:
 w=MainWindow();vp=w.viewport;vp.resize(800,600);vp.camera.aspect=800/600;vp.camera.distance=10;vp.camera.look_from(QVector3D(0,0,1.68),QVector3D(0,1,0));w._activate_tool('walk');tool=vp.active_tool
 vp.mousePressEvent(QMouseEvent(QEvent.MouseButtonPress,QPointF(400,300),QPointF(400,300),Qt.LeftButton,Qt.LeftButton,Qt.NoModifier))
 mods=Qt.ControlModifier if name.endswith('after-motion')else Qt.NoModifier
 vp.mouseMoveEvent(QMouseEvent(QEvent.MouseMove,QPointF(400,240),QPointF(400,240),Qt.NoButton,Qt.LeftButton,mods))
 vp.keyReleaseEvent(QKeyEvent(QEvent.KeyRelease,Qt.Key_Control,Qt.NoModifier))
 remembered=bool(tool._modifiers&Qt.ControlModifier);global_control=bool(QApplication.keyboardModifiers()&Qt.ControlModifier);assert not global_control
 assert tool._cursor_px==QPointF(400,240),'actual pending hover not consumed'
 tool._tick();rows.append(dict(name=name,eye=[vp.camera.eye().x(),vp.camera.eye().y(),vp.camera.eye().z()],forward=[vp.camera.forward().x(),vp.camera.forward().y(),vp.camera.forward().z()],remembered_control=remembered,global_control=global_control,undo=len(vp.history.undo_stack)));tool.on_cancel(vp);w.deleteLater()
Path(sys.argv[1]).write_text(json.dumps(dict(cases=rows,scope='actual frozen Viewport mouse/key event handlers, one actual Walk timer slot; controlled source events, not XTEST'),indent=2)+'\n')
