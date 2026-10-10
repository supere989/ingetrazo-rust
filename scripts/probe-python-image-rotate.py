import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QPointF,Qt
from PySide6.QtGui import QVector3D as V
from core.image_plane import ImagePlane
from core.layers import Layer
from tools.rotate import RotateTool
from tools.base import ToolContext
from views.viewport import Viewport
app=QApplication([]);rows={}
for case in ['selected','implicit','locked_implicit','locked_selected','hidden_selected','cancel','copy','fractional_vcb','lazy_snapshot']:
 v=Viewport();v.flash_status=lambda *a:None;v.resize(800,600);im=ImagePlane('missing',V(.125,-1.25,2.5),V(2,.25,0),V(.5,1.5,0));v.scene.image_planes=[im];im.locked=case.startswith('locked')
 if case=='hidden_selected':im.layer='Hidden';v.scene.layers.append(Layer('Hidden',visible=False))
 if case not in ['implicit','locked_implicit']:v.scene.selection={im}
 v._pixel_to_ray=lambda *a:(V(1.375,-.375,10),V(0,0,-1));t=RotateTool();v.active_tool=t
 def ctx(p):return ToolContext(v,V(*p),QPointF(),Qt.KeyboardModifier.NoModifier,None)
 def state():return {'frame':[[p.x(),p.y(),p.z()]for p in [im.origin,im.u,im.v]],'active':t.start_point is not None,'undo':len(v.history.undo_stack),'redo':len(v.history.redo_stack)}
 states=[state()];t.on_click(ctx((0,0,0)));states.append(state())
 if case=='lazy_snapshot':im.origin=im.origin+V(.7,1.2,-.3);states.append(state())
 t.on_click(ctx((1,0,0)));t.on_hover(ctx((0,1,0)));states.append(state());t.on_hover(ctx((1,1,0)));states.append(state())
 if case=='copy':t.on_key(v,Qt.Key_Control,Qt.KeyboardModifier.ControlModifier);states.append(state())
 if case=='cancel':t.on_cancel(v);states.append(state())
 else:
  if case=='fractional_vcb':t.on_value(v,33.7)
  else:t.on_click(ctx((0,1,0)))
  states.append(state())
  if v.history.undo_stack:v.history.undo();states.append(state());v.history.redo();states.append(state())
 rows[case]=states
Path(sys.argv[1]).write_text(json.dumps(rows,indent=2)+'\n')
