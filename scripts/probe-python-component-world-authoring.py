#!/usr/bin/env python3
"""Actual source container-bake + world AddFace command, root and nested."""
import sys,json
from pathlib import Path
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from core.scene import Scene
from core.group import Group,copy_group
from core.mesh import Mesh
from core.history import History,AddFaceCommand
from acceptance_context import context
rows=[]
def frame():
 t=QMatrix4x4();t.translate(1,-1,0);t.rotate(23,V(0,0,1));t.scale(2,3,1);return t
def world_vertices(group,ancestor=None):
 t=(ancestor or QMatrix4x4())*(group.xform or QMatrix4x4())
 return [list(t.map(v.position).toTuple()) for v in group.mesh.vertices]
for nested in (False,True):
 s=Scene();h=History(s);parent=Group(name="authored container");parent.component=True;parent.xform=frame() if not nested else QMatrix4x4()
 if nested:parent.xform.translate(2,1,0)
 childmesh=Mesh();childmesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)]);parent.children=[Group(childmesh,name="existing child")]
 sibling=copy_group(parent,V(10,0,0))
 if nested:
  outer=Group(name="outer");outer.xform=frame();outer.component=True;outer.children=[parent,sibling];s.groups=[outer];s.begin_group_edit(outer)
 else:s.groups=[parent,sibling]
 world_frame=QMatrix4x4(parent.xform);points=[world_frame.map(p) for p in (V(0,0,0),V(1,0,0),V(0,1,0))]
 sibling_before_matrix=list(sibling.xform.data());sibling_child_before=world_vertices(sibling.children[0],sibling.xform)
 s.begin_group_edit(parent);h.execute(AddFaceCommand(points));assert not h.last_error
 s.end_group_edit();assert parent.mesh is sibling.mesh
 assert list(sibling.xform.data())==sibling_before_matrix
 assert world_vertices(sibling.children[0],sibling.xform)==sibling_child_before
 rows.append({"nested":nested,"world_frame":list(world_frame.data()),"authored_world_points":[list(p.toTuple()) for p in points],"selected_world_vertices":world_vertices(parent),"sibling_world_vertices":world_vertices(sibling),"selected_child_world_vertices":world_vertices(parent.children[0],parent.xform),"sibling_child_world_vertices":world_vertices(sibling.children[0],sibling.xform),"shared_own_mesh":True,"selected_matrix":list(parent.xform.data()),"sibling_matrix":list(sibling.xform.data()),"source_history_entries":len(h.undo_stack)})
Path("docs/audits/component-world-authoring-source.json").write_text(json.dumps({"provenance":context(),"scope":"Actual original Scene nested/root edit entry and AddFaceCommand with frozen worldpoints; world surfaces and shared prototype identity after exit, not rendered gestures.","cases":rows},indent=2)+"\n")
print("Original transformed-empty root/nested world-authoring controls PASS")
