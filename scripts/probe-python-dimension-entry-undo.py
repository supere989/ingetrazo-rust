#!/usr/bin/env python3
"""Actual original entry/history law used to repair the stale CI fixture."""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.viewport import Viewport
from core.group import Group
from core.mesh import Mesh
from core.dimension import Dimension
from core.history import MoveVerticesCommand,RenameGroupCommand,AddDimensionCommand
from acceptance_context import context
app=QApplication([])
def mat(x,y,z):m=M();m.translate(x,y,z);return m
def wire():m=Mesh();m.add_edge(V(),V(1,0,0));return m
def points(mesh):return [[v.position.x(),v.position.y(),v.position.z()]for v in mesh.vertices]
def viewport():vp=Viewport();vp.flash_status=lambda *a:None;return vp
rows={}
vp=viewport();g=Group(wire(),'Translated');g.xform=mat(10,20,30);vp.scene.groups=[g];vp.scene.dimensions=[Dimension(V(10,20,30),V(11,20,30),V(0,1,0))];vp.scene.dimensions[0].bind(vp.scene)
vp.begin_group_edit(g);entry=points(vp.scene.mesh);assert entry==[[10,20,30],[11,20,30]] and [1,0,0]not in entry
vp.history.execute(MoveVerticesCommand([V(11,20,30)],V(3,0,0)));vp.history.execute(AddDimensionCommand(Dimension(V(10,20,30),V(14,20,30),V(0,2,0),text='Local <>')));vp.end_one_group_edit();assert points(g.mesh)==[[0,0,0],[4,0,0]] and len(vp.scene.dimensions)==2
vp.history.undo();assert points(g.mesh)==[[0,0,0],[1,0,0]] and len(vp.scene.dimensions)==2
vp.history.redo();assert points(g.mesh)==[[0,0,0],[4,0,0]] and len(vp.scene.dimensions)==2
rows['translated']={'entered_vertices':entry,'local_vertex_negative_rejected':True,'shareback_local_vertices':points(g.mesh),'original_undo_redo_dimension_count':[2,2],'source_annotation_not_folded_into_mesh_undo':True}
vp=viewport();leaf=Group(wire(),'Leaf');leaf.xform=mat(1,2,3);middle=Group(name='Middle');middle.xform=mat(10,20,30);middle.children=[leaf];parent=Group(name='Parent');parent.xform=mat(100,200,300);parent.children=[middle];vp.scene.groups=[parent];vp.scene.dimensions=[Dimension(V(111,222,333),V(112,222,333),V(0,1,0))];vp.scene.dimensions[0].bind(vp.scene)
vp.begin_group_edit(parent);vp.begin_group_edit(middle);vp.history.execute(RenameGroupCommand(leaf,'Edited leaf'));vp.begin_group_edit(leaf);entry=points(vp.scene.mesh);assert entry==[[111,222,333],[112,222,333]]
vp.history.execute(MoveVerticesCommand([V(112,222,333)],V(5,0,0)));vp.end_one_group_edit();vp.end_one_group_edit();vp.end_one_group_edit();assert points(leaf.mesh)==[[0,0,0],[6,0,0]]
vp.history.undo();assert points(leaf.mesh)==[[0,0,0],[1,0,0]] and leaf.name=='Edited leaf';assert parent.xform==M() and middle.xform==M() and leaf.xform==mat(111,222,333)
vp.history.undo();assert leaf.name=='Leaf';vp.history.redo();assert leaf.name=='Edited leaf';vp.history.redo();assert points(leaf.mesh)==[[0,0,0],[6,0,0]]
rows['nested']={'entered_vertices':entry,'container_bakes_survive_undo':True,'undo_preserves_prior_rename':True,'separate_rename_undo_redo':True,'leaf_restored_placement':list(leaf.xform.data())}
report={'provenance':context(),'passed':True,'cases':rows,'scope':'Actual original Viewport begin/end and History commands, no rendered UI or key dispatch claim'}
Path('docs/audits/dimension-ci-entry-original.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
