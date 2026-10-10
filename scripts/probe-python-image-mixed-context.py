#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QPointF,Qt
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from core.image_plane import ImagePlane
from core.mesh import Mesh
from core.group import Group,iter_placements
from core.history import MoveVerticesCommand
from tools.move import MoveTool
from tools.base import ToolContext
from views.viewport import Viewport
from acceptance_context import context
app=QApplication([]);rows={}
for context_kind in ['plain','component','folded','nested','nested_folded']:
 for case in ['finish_live','cancel_then_finish','finish_coincident_root']:
  v=Viewport();v.flash_status=lambda *a:None;mesh=Mesh();mesh.add_face([V(),V(1,0,0),V(0,1,0)]);g=Group(mesh,'Leaf');g.component=context_kind!='plain';g.xform=M();
  if context_kind.startswith('nested'):
   outer=Group(name='Outer');outer.children=[g];outer.component=True;outer.xform=M();v.scene.groups=[outer]
  else:v.scene.groups=[g]
  if case=='finish_coincident_root':v.scene.mesh.add_face([V(),V(1,0,0),V(0,1,0)])
  im=ImagePlane('scan',V(.125,-1.25,2.5),V(2,0,0),V(0,2,0));v.scene.image_planes=[im];
  if context_kind.startswith('nested'):v.begin_group_edit(outer)
  v.begin_group_edit(g);
  if context_kind in ['folded','nested_folded']:v.history.execute(MoveVerticesCommand([V(v.scene.mesh.vertices[0].position)],V(0,0,.25)))
  v.scene.selection={im,v.scene.mesh.faces[0]};tool=MoveTool();v.active_tool=tool
  def ctx(p):return ToolContext(v,V(*p),QPointF(),Qt.KeyboardModifier.NoModifier,None)
  def geometry():
   values=[x.position for x in v.scene.mesh.vertices]
   for root in v.scene.groups:
    for group,xf in iter_placements(root):
     if group is not v.scene.edit_group:values.extend(xf.map(x.position)if xf is not None else x.position for x in group.mesh.vertices)
   return sorted([[p.x(),p.y(),p.z()]for p in values])
  def state():return {'origin':[im.origin.x(),im.origin.y(),im.origin.z()],'active':v.active_tool is tool and tool.start_point is not None,'editing':v.scene.edit_group is not None,'undo':len(v.history.undo_stack),'geometry':geometry()}
  states=[state()];tool.on_click(ctx((0,0,0)));tool.on_hover(ctx((1,2,0)));tool.on_hover(ctx((2,3,0)));states.append(state())
  if case=='cancel_then_finish':tool.on_cancel(v);states.append(state())
  v.end_group_edit();states.append(state())
  if case in ['finish_live','finish_coincident_root']:
   tool.on_hover(ctx((3,4,0)));states.append(state());tool.on_click(ctx((5,2,0)));states.append(state());v.history.undo();states.append(state());v.history.redo();states.append(state())
  v.active_tool.on_cancel(v);states.append(state());rows[context_kind+':'+case]=states
Path(sys.argv[1]).write_text(json.dumps({'provenance':context(),'cases':rows},indent=2)+'\n')
