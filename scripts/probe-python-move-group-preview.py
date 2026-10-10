#!/usr/bin/env python3
"""Actual original group-only Move controller and frozen-preview state boundary."""
import os,sys,json
from pathlib import Path
from types import SimpleNamespace as N
os.environ.setdefault('QT_QPA_PLATFORM','xcb')
sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from PySide6.QtCore import QPointF
from core.group import Group
from core.mesh import Mesh
from views.viewport import Viewport
from tools.move import MoveTool
from acceptance_context import context
app=QApplication([])
rows=[]
for placed in (False,True):
 vp=Viewport();vp.resize(800,600);vp.show();app.processEvents();vp.makeCurrent();vp.flash_status=lambda text:None
 mesh=Mesh();mesh.add_face([V(0,0,0),V(1,0,0),V(1,1,0),V(0,1,0)])
 group=Group(mesh,name='moving');vp.scene.groups.append(group)
 if placed:
  group.xform=QMatrix4x4();group.xform.translate(0,2,0);group.xform.rotate(17,V(0,0,1))
 before=[tuple(v.position.toTuple()) for v in mesh.vertices]
 matrix=None if group.xform is None else list(group.xform.data())
 vp.scene.select([group]);tool=MoveTool();vp.set_active_tool(tool)
 grab=N(viewport=vp,world=V(0,0,0),screen=QPointF(0,0))
 tool.on_click(grab);version=vp.scene.version
 assert tool._vp_preview
 for x in (1.,2.,3.):
  tool.on_hover(N(viewport=vp,world=V(x,0,0),screen=QPointF(0,0)))
  assert vp.scene.version==version and not vp.history.undo_stack
  assert [tuple(v.position.toTuple()) for v in mesh.vertices]==before
  assert (None if group.xform is None else list(group.xform.data()))==matrix
  assert vp._preview_offset==V(x,0,0)
 tool.on_cancel(vp)
 assert [tuple(v.position.toTuple()) for v in mesh.vertices]==before
 assert (None if group.xform is None else list(group.xform.data()))==matrix
 vp.close()
 rows.append({'placed':placed,'pointer_recipe_offsets':[1,2,3],'frozen_scene_version':version,'geometry_unchanged':True,'matrix_unchanged':True,'undo':0,'cancel_restores':True})
# Operative placed component copy/retype: definitions shared, placements fresh.
vp=Viewport();vp.resize(800,600);vp.show();app.processEvents();vp.makeCurrent();vp.flash_status=lambda text:None
mesh=Mesh();mesh.add_face([V(0,0,0),V(1,0,0),V(1,1,0),V(0,1,0)])
group=Group(mesh,name='linked');group.xform=QMatrix4x4();group.xform.translate(0,2,0);group.xform.rotate(17,V(0,0,1));group.component=True
group.children.append(Group(name='nested private'));vp.scene.groups.append(group);vp.scene.select([group]);tool=MoveTool();vp.set_active_tool(tool)
tool.on_click(N(viewport=vp,world=V(0,0,0),screen=QPointF(0,0)));tool._copy=True;tool.hover_point=V(3,0,0)
assert tool.on_array_value(vp,3,'x',step=2.)
assert len(vp.history.undo_stack)==1 and len(vp.scene.groups)==4
assert all(g.mesh is mesh for g in vp.scene.groups)
assert len({g.uid for g in vp.scene.groups})==4
assert len({g.children[0].uid for g in vp.scene.groups})==4
assert all(g.children[0].mesh is group.children[0].mesh for g in vp.scene.groups)
assert [g.xform.map(V(0,0,0)).x() for g in vp.scene.groups]==[0.,2.,4.,6.]
assert tool.on_array_value(vp,2,'x',step=4.)
assert [g.xform.map(V(0,0,0)).x() for g in vp.scene.groups]==[0.,4.,8.]
assert len(vp.history.undo_stack)==1
vp.history.undo();assert len(vp.scene.groups)==1 and vp.scene.groups[0] is group
vp.history.redo();assert len(vp.scene.groups)==3
rows.append({'name':'linked nested group spaced array retype','first_offsets':[0,2,4,6],'retyped_offsets':[0,4,8],'shared_parent_and_child_mesh':True,'fresh_parent_and_child_uids':True,'undo_entries':1,'undo_groups':1,'redo_groups':3})
vp.close()
Path('docs/audits/move-group-frozen-source-controls.json').write_text(json.dumps({'provenance':context(),'cases':rows,'scope':'Operative original Move.on_click/on_hover/on_cancel and Viewport frozen-preview state. Controlled world/screen recipes with a real isolated software GL context, no rendered pointer/GPU upload or timing acceptance.'},indent=2)+'\n')
print('Original ordinary/placed group frozen preview controls PASS')
