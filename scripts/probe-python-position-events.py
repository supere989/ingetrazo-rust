#!/usr/bin/env python3
"""Actual frozen viewport plane/snap and Position handlers, not rendered-input proof."""
import json,sys,tempfile
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ingetrazo'))
from PySide6.QtCore import QPointF,Qt,QSettings
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D
from views.main_window import MainWindow
from core.history import History
from core.scene import Scene
from core.group import Group
from core.layers import Layer
from core.image_plane import ImagePlane
from tools.walkthrough import PositionCameraTool,set_eye_height
from tools.base import ToolContext
app=QApplication([]);app.setOrganizationName('IngeTrazoIndependentPosition');app.setApplicationName('isolated')
QSettings.setDefaultFormat(QSettings.IniFormat);QSettings.setPath(QSettings.IniFormat,QSettings.UserScope,tempfile.mkdtemp())
set_eye_height(1.68)
def vec(v):return [v.x(),v.y(),v.z()]
class Event:
 def __init__(self,x,y):self.p=QPointF(x,y)
 def position(self):return self.p
 def modifiers(self):return Qt.NoModifier
rows=[]
for name in ['split-drag','move-release-one-frame','press-move-release-one-frame','six-pixel-click']:
 window=MainWindow();vp=window.viewport;vp.resize(800,600);vp.scene=Scene();vp.history=History(vp.scene);vp.camera.aspect=800/600;vp.camera.distance=10;vp.camera.target=QVector3D();vp.camera.perspective=False;vp.camera.set_view('top');window._activate_tool('position_camera');tool=vp.active_tool
 tool.on_click(vp._build_ctx(Event(430,330)));tool.on_hover(vp._build_ctx(Event(436,330)if name=='six-pixel-click'else Event(470,350)));tool.on_release(vp)
 rows.append(dict(name=name,eye=vec(vp.camera.eye()),forward=vec(vp.camera.forward()),mode=type(vp.active_tool).__name__,undo=len(vp.history.undo_stack)));window.deleteLater()
Path(sys.argv[1]).write_text(json.dumps(dict(cases=rows,scope='Actual frozen _build_ctx/Position tool handlers, same source event order; native batching is egui-only'),indent=2)+'\n')
