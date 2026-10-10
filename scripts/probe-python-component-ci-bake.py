#!/usr/bin/env python3
"""Frozen actual original nested container WORLD entry, inverse shareback and Undo."""
import os,sys,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.viewport import Viewport
from core.mesh import Mesh
from core.group import Group,copy_group
app=QApplication([]);rows=[]
frames=[("translated",M(1,0,0,4,0,1,0,5,0,0,1,6,0,0,0,1)),("rotated",M(0,-1,0,4,1,0,0,5,0,0,1,6,0,0,0,1)),("tilted_scaled",M(2,0,0,4,0,0,-1,5,0,3,0,6,0,0,0,1))]
def matrix(g):return list((g.xform if g.xform is not None else M()).data())
def points(g):return[list(v.position.toTuple())for v in g.mesh.vertices]
for name,frame in frames:
 vp=Viewport();vp.flash_status=lambda *a:None;m=Mesh();m.add_face([V(0,0,0),V(1,0,0),V(0,1,0)])
 leaf=Group(m,name="first");leaf.component=True;leaf.xform=M();other=copy_group(leaf,V());plain=copy_group(leaf,V());plain.component=False
 child=copy_group(leaf,V());child.xform=M();child.xform.translate(-2,1,0)
 parent=Group(name="parent");parent.component=True;parent.xform=M(frame);parent.children=[child];sibling=copy_group(parent,V(10,10,10));vp.scene.groups=[leaf,other,plain,parent,sibling]
 before=dict(selected=matrix(parent),child=matrix(child),sibling=matrix(sibling),sibling_child=matrix(sibling.children[0]));combined=frame*child.xform
 vp.begin_group_edit(parent);parent_drawing_frame=list(vp.scene.drawing_frame.data());parent_entry=dict(selected=matrix(parent),child=matrix(child),history=len(vp.history.undo_stack));assert parent.xform==M();assert child.xform==combined
 vp.begin_group_edit(child);child_drawing_frame=list(vp.scene.drawing_frame.data());entry=points(child);target=combined.map(V(0,0,5));child.mesh.vertices[0].position=target;vp.end_one_group_edit();vp.end_one_group_edit()
 committed=dict(selected=matrix(parent),child=matrix(child),sibling=matrix(sibling),sibling_child=matrix(sibling.children[0]),prototypes=[points(g)for g in [leaf,other,plain,child,sibling.children[0]]],history=len(vp.history.undo_stack),shared=all(g.mesh is leaf.mesh for g in [other,plain,child,sibling.children[0]]))
 assert committed["prototypes"][0][0]==[0.,0.,5.];assert committed["shared"];assert committed["history"]==1
 vp.history.undo();undone=dict(selected=matrix(parent),child=matrix(child),points=points(leaf));assert parent.xform==M() and child.xform==combined and points(leaf)[0]==[0.,0.,0.]
 rows.append(dict(name=name,parent_drawing_frame=parent_drawing_frame,child_drawing_frame=child_drawing_frame,before=before,parent_entry=parent_entry,entry_world_points=entry,authored_world_point=list(target.toTuple()),committed=committed,undone=undone))
out=Path("docs/audits/component-ci-world-bake");out.mkdir(exist_ok=True);(out/"original.json").write_text(json.dumps({"original_pin":"6be29fe437a7cd992d4e079f71720821b25216b9","cases":rows},indent=2)+"\n");print("PASS original translated/rotated/tilted scaled nested world-bake, shared plain prototype and outside-history Undo")
