#!/usr/bin/env python3
"""Real original QTimer overruns; no replacement tick or artificial emission."""
import json,sys,time,tempfile
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ingetrazo'))
from PySide6.QtCore import QPointF,Qt,QSettings
from PySide6.QtGui import QVector3D
from PySide6.QtWidgets import QApplication
from views.main_window import MainWindow
from tools.walkthrough import set_eye_height
app=QApplication([]);app.setOrganizationName('NavigationTimerIndependent');app.setApplicationName('isolated')
QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,tempfile.mkdtemp());set_eye_height(1.68)
class Event:
 def __init__(self,x,y):self.p=QPointF(x,y)
 def position(self):return self.p
 def modifiers(self):return Qt.NoModifier
rows=[]
for mode,key,delay in [('walk','walk',.170),('first-person','first_person',.170)]:
 w=MainWindow();vp=w.viewport;vp.resize(800,600);vp.camera.aspect=4/3;vp.camera.distance=10;vp.camera.look_from(QVector3D(0,0,1.68),QVector3D(0,1,0));vp.camera.perspective=True
 w._activate_tool(key);t=vp.active_tool;count=[0]
 if mode=='walk':t.on_click(vp._build_ctx(Event(400,300)));t.on_hover(vp._build_ctx(Event(400,240)))
 else:assert t.on_key(vp,Qt.Key_W,Qt.NoModifier)
 t._timer.timeout.connect(lambda:count.__setitem__(0,count[0]+1))
 before=vp.camera.eye();start=time.monotonic();time.sleep(delay);elapsed=time.monotonic()-start;app.processEvents();after=vp.camera.eye();t.on_cancel(vp)
 rows.append(dict(name=mode+'-stalled',blocked_seconds=elapsed,timeouts=count[0],before=[before.x(),before.y(),before.z()],after=[after.x(),after.y(),after.z()],delta=[after.x()-before.x(),after.y()-before.y(),after.z()-before.z()],timer_active=t._timer.isActive()))
 assert count[0]==1,rows[-1]
 w.deleteLater();app.processEvents()
Path(sys.argv[1]).write_text(json.dumps(dict(cases=rows,scope='actual QTimer dispatch after one blocked event-loop interval; controlled handler setup, not rendered input'),indent=2)+'\n')
