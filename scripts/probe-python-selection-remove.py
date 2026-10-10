#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QPointF,Qt
from PySide6.QtGui import QVector3D as V
from core.mesh import Mesh
from core.group import Group
from tools.select import SelectTool
from tools.base import ToolContext
from views.viewport import Viewport
from acceptance_context import context
app=QApplication([]);rows={}
for kind in ['Face','Edge','Group']:
 for selected in [False,True]:
  vp=Viewport();mesh=Mesh();face=mesh.add_face([V(),V(2,0,0),V(0,2,0)])
  target=face if kind=='Face'else mesh.add_edge(V(0,0,0),V(2,0,0))if kind=='Edge'else Group(mesh)
  vp.scene.mesh=mesh
  if kind=='Group':vp.scene.groups.append(target)
  if selected:vp.scene.selection={target}
  tool=SelectTool();tool._pick=lambda *a:target
  tool.on_click(ToolContext(vp,V(),QPointF(),Qt.KeyboardModifier.ControlModifier|Qt.KeyboardModifier.ShiftModifier,None))
  rows[kind+('_selected'if selected else'_unselected')]={'selected':target in vp.scene.selection,'count':len(vp.scene.selection)}
Path(sys.argv[1]).write_text(json.dumps({'provenance':context(),'cases':rows},indent=2)+'\n')
