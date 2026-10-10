#!/usr/bin/env python3
import os,sys,json,gc
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QPointF,Qt
from PySide6.QtGui import QVector3D as V
from core.image_plane import ImagePlane
from tools.move import MoveTool
from tools.base import ToolContext
from views.viewport import Viewport
from acceptance_context import context
app=QApplication([]);v=Viewport();im=ImagePlane('scan',V(),V(2,0,0),V(0,2,0));identity=id(im);v.scene.image_planes=[im];v.scene.selection={im};tool=MoveTool();v.active_tool=tool;tool.on_click(ToolContext(v,V(),QPointF(),Qt.KeyboardModifier.NoModifier,None));del im
v.scene.image_planes.clear();v.scene.selection.clear();v.history.undo_stack.clear();v.history.redo_stack.clear();gc.collect();alive=any(id(x)==identity for x in gc.get_objects());tool.on_cancel(v);gc.collect();released=not any(id(x)==identity for x in gc.get_objects())
Path(sys.argv[1]).write_text(json.dumps({'provenance':context(),'cases':{'detached_tool_owner':alive,'released_after_cancel':released}},indent=2)+'\n')
