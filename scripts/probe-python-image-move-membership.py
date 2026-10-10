import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QPointF,Qt
from PySide6.QtGui import QVector3D as V
from core.image_plane import ImagePlane
from core.history import DeleteImagePlanesCommand
from tools.move import MoveTool
from tools.base import ToolContext
from views.viewport import Viewport
app=QApplication([]);rows={}
for mutation in ['reverse','delete_target','delete_other']:
 for end in ['cancel','commit']:
  v=Viewport();v.flash_status=lambda *a:None;a=ImagePlane('A',V(.125,0,0),V(2,0,0),V(0,2,0));b=ImagePlane('B',V(10,0,0),V(2,0,0),V(0,2,0));v.scene.image_planes=[a,b];v.scene.selection={a};t=MoveTool();v.active_tool=t
  def ctx(p):return ToolContext(v,V(*p),QPointF(),Qt.KeyboardModifier.NoModifier,None)
  def state():return {'live':[[p.path,p.origin.x(),p.origin.y(),p.origin.z()]for p in v.scene.image_planes],'held':[a.origin.x(),a.origin.y(),a.origin.z()]}
  states=[state()];t.on_click(ctx((0,0,0)));t.on_hover(ctx((1,2,0)));states.append(state())
  cmd=None
  if mutation=='reverse':v.scene.image_planes.reverse()
  else:cmd=DeleteImagePlanesCommand([a if mutation=='delete_target'else b]);cmd.do(v.scene)
  states.append(state());t.on_hover(ctx((3,4,0)));states.append(state())
  if end=='cancel':t.on_cancel(v)
  else:t.on_click(ctx((5,6,0)))
  states.append(state())
  if cmd:cmd.undo(v.scene)
  states.append(state());rows[mutation+':'+end]=states
Path(sys.argv[1]).write_text(json.dumps(rows,indent=2)+'\n')
