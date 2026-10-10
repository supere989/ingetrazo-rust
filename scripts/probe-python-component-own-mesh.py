#!/usr/bin/env python3
"""Actual source container entry determines private versus shared own Mesh."""
import sys,json
from pathlib import Path
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from core.scene import Scene
from core.group import Group,copy_group
from core.mesh import Mesh
from acceptance_context import context
rows=[]
for transformed in (False,True):
 for nonempty in (False,True):
  s=Scene();mesh=Mesh()
  if nonempty:mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)])
  parent=Group(mesh,name="container");parent.component=True;parent.xform=QMatrix4x4()
  if transformed:parent.xform.translate(1,-1,0);parent.xform.rotate(23,V(0,0,1));parent.xform.scale(2,3,1)
  parent.children.append(Group(name="child"));s.groups.append(parent);sibling=copy_group(parent,V(10,0,0));s.groups.append(sibling)
  original=parent.mesh;before=[list(v.position.toTuple()) for v in sibling.mesh.vertices]
  s.begin_group_edit(parent);private=parent.mesh is not sibling.mesh
  if nonempty:parent.mesh.vertices[0].position+=V(0,0,2)
  else:s.mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)])
  s.end_group_edit()
  assert private==(transformed and nonempty)
  if private:assert [list(v.position.toTuple()) for v in sibling.mesh.vertices]==before
  else:assert parent.mesh is sibling.mesh
  rows.append({"transformed":transformed,"nonempty":nonempty,"private_mesh_after_entry":private,"original_prototype_retained_by_sibling":sibling.mesh is original,"selected_vertices":[list(v.position.toTuple()) for v in parent.mesh.vertices],"sibling_vertices":[list(v.position.toTuple()) for v in sibling.mesh.vertices],"sibling_before":before})
Path("docs/audits/component-own-mesh-source.json").write_text(json.dumps({"provenance":context(),"scope":"Actual Scene.begin_group_edit/_bake_container_xform/end_group_edit; direct ownMesh vertex edit or first face addition; no viewport gestures/History acceptance.","cases":rows},indent=2)+"\n")
print("Original identity/transformed empty/nonempty container ownMesh controls PASS")
