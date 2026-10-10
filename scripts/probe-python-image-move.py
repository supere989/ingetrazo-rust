#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QPointF,Qt
from PySide6.QtGui import QVector3D as V
from core.image_plane import ImagePlane
from core.layers import Layer
from tools.move import MoveTool
from tools.base import ToolContext
from views.viewport import Viewport
from acceptance_context import context
app=QApplication([]);rows={}
for case in ['selected','implicit','locked_implicit','locked_selected','hidden_selected','cancel','copy','fractional']:
 v=Viewport();v.flash_status=lambda *a:None;v.resize(800,600);im=ImagePlane('missing',V(),V(2,0,0),V(0,2,0));v.scene.image_planes.append(im)
 if case=='fractional':im.origin=V(.125,-1.25,2.5)
 im.locked=case.startswith('locked')
 if case=='hidden_selected':im.layer='Hidden';v.scene.layers.append(Layer('Hidden',visible=False))
 if case not in ['implicit','locked_implicit']:v.scene.selection={im}
 v._pixel_to_ray=lambda *a:(V(.5,.5,10),V(0,0,-1));tool=MoveTool();v.active_tool=tool
 def ctx(p):return ToolContext(v,V(*p),QPointF(),Qt.KeyboardModifier.NoModifier,None)
 def state():return {'origin':[im.origin.x(),im.origin.y(),im.origin.z()],'active':tool.start_point is not None,'undo':len(v.history.undo_stack),'redo':len(v.history.redo_stack),'images':len(v.scene.image_planes)}
 states=[state()];tool.on_click(ctx((0,0,0)));states.append(state());tool.on_hover(ctx((.7,1.2,-.3)if case=='fractional'else(2,3,0)));states.append(state())
 if case=='fractional':tool.on_hover(ctx((.9,.4,.8)));states.append(state())
 if case=='copy':tool.on_key(v,Qt.Key_Control,Qt.KeyboardModifier.ControlModifier);states.append(state())
 if case=='cancel':tool.on_cancel(v);states.append(state())
 else:
  tool.on_click(ctx((.6,.8,-.2)if case=='fractional'else(4,1,0)));states.append(state())
  if v.history.undo_stack:
   im.origin=im.origin+V(10,0,0);states.append(state());v.history.undo();states.append(state());v.history.redo();states.append(state())
 rows[case]=states
Path(sys.argv[1]).write_text(json.dumps({'provenance':context(),'cases':rows},indent=2)+'\n')
