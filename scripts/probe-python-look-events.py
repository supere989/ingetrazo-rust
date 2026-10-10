#!/usr/bin/env python3
import sys,json
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ingetrazo'))
from PySide6.QtCore import QPointF,Qt
from PySide6.QtGui import QVector3D
from PySide6.QtWidgets import QApplication
from views.main_window import MainWindow
app=QApplication([])
class Event:
 def __init__(self,x,y):self.p=QPointF(x,y)
 def position(self):return self.p
 def modifiers(self):return Qt.NoModifier
rows=[]
for name in ['look-move-release','look-press-move-release']:
 w=MainWindow();vp=w.viewport;vp.resize(800,600);vp.camera.aspect=4/3;vp.camera.distance=10;vp.camera.look_from(QVector3D(0,0,1.68),QVector3D(0,1,0));vp.camera.perspective=True;w._activate_tool('look_around');tool=vp.active_tool
 before=vp.camera.eye();tool.on_click(vp._build_ctx(Event(400,300)));tool.on_hover(vp._build_ctx(Event(440,260)));tool.on_release(vp)
 rows.append(dict(name=name,eye=[vp.camera.eye().x(),vp.camera.eye().y(),vp.camera.eye().z()],before_eye=[before.x(),before.y(),before.z()],yaw=vp.camera.yaw,pitch=vp.camera.pitch,undo=len(vp.history.undo_stack)));w.deleteLater();app.processEvents()
Path(sys.argv[1]).write_text(json.dumps(dict(cases=rows,scope='actual frozen MainWindow/tool contexts/handlers; two native batching arrangements compare same ordered source commands, not XTEST timing'),indent=2)+'\n')
