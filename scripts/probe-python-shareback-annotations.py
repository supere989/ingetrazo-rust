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
from core.history import MoveVerticesCommand,RenameGroupCommand,AddDimensionCommand,PlaceSectionPlaneCommand
from core.section import SectionPlane
from acceptance_context import context
app=QApplication([])
def mat(x,y,z):m=M();m.translate(x,y,z);return m
def wire():m=Mesh();m.add_edge(V(),V(1,0,0));return m
def points(mesh):return [[v.position.x(),v.position.y(),v.position.z()]for v in mesh.vertices]
def viewport():vp=Viewport();vp.flash_status=lambda *a:None;return vp
rows={}
def state(vp,g):
 return {'mesh':points(g.mesh),'dimensions':[{'a':[d.a.x(),d.a.y(),d.a.z()],'b':[d.b.x(),d.b.y(),d.b.z()],'offset':[d.offset.x(),d.offset.y(),d.offset.z()],'text':d.text,'axis':d.axis,'anchor_a':d.anchor_a is not None,'anchor_b':d.anchor_b is not None} for d in vp.scene.dimensions]}
vp=viewport();g=Group(wire(),'Translated');g.xform=mat(10,20,30);vp.scene.groups=[g];vp.scene.dimensions=[Dimension(V(10,20,30),V(11,20,30),V(0,1,0))];vp.scene.dimensions[0].bind(vp.scene)
vp.begin_group_edit(g)
vp.history.execute(MoveVerticesCommand([V(11,20,30)],V(3,0,0)))
vp.history.execute(AddDimensionCommand(Dimension(V(10,20,30),V(14,20,30),V(0,2,0),text='Local <>')))
vp.end_one_group_edit();rows['finish']=state(vp,g)
for i in range(2):
 vp.history.undo();rows['undo'+str(i)]=state(vp,g)
 vp.history.redo();rows['redo'+str(i)]=state(vp,g)
vp=viewport();g=Group(wire(),'Annotation only');g.xform=mat(10,20,30);vp.scene.groups=[g];vp.begin_group_edit(g)
vp.history.execute(AddDimensionCommand(Dimension(V(10,20,30),V(11,20,30),V(0,2,0),text='Only <>')));vp.end_one_group_edit();counts=[len(vp.scene.dimensions)];assert type(vp.history.undo_stack[-1]).__name__=='AddDimensionCommand'
vp.history.undo();counts.append(len(vp.scene.dimensions));vp.history.redo();counts.append(len(vp.scene.dimensions));rows['annotation_only_history']=counts
vp=viewport();g=Group(wire(),'Section');g.xform=mat(10,20,30);vp.scene.groups=[g];vp.begin_group_edit(g);vp.history.execute(MoveVerticesCommand([V(11,20,30)],V(3,0,0)));plane=SectionPlane(V(10,20,30),V(0,0,1));vp.history.execute(PlaceSectionPlaneCommand(plane));vp.end_one_group_edit();counts=[len(vp.scene.section_planes)];vp.history.undo();counts.append(len(vp.scene.section_planes));vp.history.redo();counts.append(len(vp.scene.section_planes));rows['section_retained']=counts
report={'provenance':context(),'cases':rows}
print(json.dumps(report,indent=2));Path('/tmp/original-shareback-annotations.json').write_text(json.dumps(report,indent=2)+'\n')
