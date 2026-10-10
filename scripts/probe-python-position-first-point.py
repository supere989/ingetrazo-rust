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
for name,view,kind,drag,cancel in [
 ('front-empty','front','empty',None,False),('right-empty','right','empty',None,False),
 ('top-empty','top','empty',None,False),('iso-ground','iso','empty',None,False),
 ('front-locked-image','front','image',None,False),('face-before-image','front','face-image',None,False),
 ('position-drag','top','empty',(470,350),False),('position-cancel','top','empty',(470,350),True),
 ('position-six-pixels','top','empty',(436,330),False),
 ('loose-orphan','front','orphan',None,False),('visible-edge-bounds','front','visible-edge',None,False),
 ('draw-hidden-edge-bounds','front','draw-hidden-edge',None,False),('hidden-layer-bounds','front','hidden-layer',None,False),
 ('hidden-group-bounds','front','hidden-group',None,False),('group-orphan-bounds','front','group-orphan',None,False),
 ('hidden-nested-descendant-bounds','front','hidden-nested',None,False)]:
 window=MainWindow();vp=window.viewport;vp.resize(800,600);vp.scene=Scene();vp.history=History(vp.scene);vp.camera.aspect=800/600;vp.camera.distance=10;vp.camera.target=QVector3D();vp.camera.perspective=False;vp.camera.set_view(view)
 if 'image' in kind:vp.scene.image_planes.append(ImagePlane('scan',QVector3D(-4,2,-4),QVector3D(8,0,0),QVector3D(0,0,8),locked=True))
 if kind=='face-image':vp.scene.loose_mesh.add_face([QVector3D(-4,1,-4),QVector3D(4,1,-4),QVector3D(4,1,4),QVector3D(-4,1,4)])
 if kind=='orphan':vp.scene.loose_mesh.vertex(QVector3D(100,200,300))
 if kind in ('visible-edge','draw-hidden-edge','hidden-layer','hidden-group'):
  edge=vp.scene.loose_mesh.add_edge(QVector3D(5,4,0),QVector3D(6,4,1))
  if kind=='draw-hidden-edge':edge.hidden=True
  if kind=='hidden-layer':
   vp.scene.layers.append(Layer('off',visible=False));edge2=vp.scene.loose_mesh.add_edge(QVector3D(100,200,300),QVector3D(101,201,301));edge2.layer='off'
  if kind=='hidden-group':
   g=Group();g.hidden=True;g.mesh.vertex(QVector3D(100,200,300));vp.scene.groups.append(g)
 if kind=='group-orphan':
  g=Group();g.mesh.vertex(QVector3D(50,8,20));vp.scene.groups.append(g)
 if kind=='hidden-nested':
  g=Group();child=Group();child.hidden=True;child.mesh.vertex(QVector3D(100,200,300));grand=Group();grand.mesh.vertex(QVector3D(50,12,20));child.children=[grand];g.children=[child];vp.scene.groups.append(g)
 window._activate_tool("position_camera");tool=vp.active_tool
 bounds=vp.scene.bounds();before=vec(vp.camera.eye());ctx=vp._build_ctx(Event(430,330));assert ctx is not None,name
 tool.on_click(ctx)
 if drag is not None:tool.on_hover(vp._build_ctx(Event(*drag)))
 preview=[[vec(a),vec(b)]for a,b in tool.rubber_band_lines()]
 if cancel:tool.on_cancel(vp)
 else:tool.on_release(vp)
 rows.append(dict(name=name,bounds=[vec(v) for v in bounds] if bounds[0] is not None else None,point=vec(ctx.world),snap=ctx.snap.kind,preview=preview,after_eye=vec(vp.camera.eye()),after_forward=vec(vp.camera.forward()),before_eye=before,cancelled=cancel,undo=len(vp.history.undo_stack),mode=type(vp.active_tool).__name__))
 window.deleteLater()
Path(sys.argv[1]).write_text(json.dumps({'cases':rows,'scope':'Actual frozen Viewport._build_ctx and PositionCameraTool handlers; controlled cursor recipes, not XTEST/rendered acceptance'},indent=2)+'\n')
