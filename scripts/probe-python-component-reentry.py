#!/usr/bin/env python3
"""Actual original no-edit History and axes after container exit/reentry."""
import sys,json
from pathlib import Path
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from core.scene import Scene
from core.group import Group,copy_group,group_frame
from core.history import History
from core.mesh import Mesh
from core import axes
from acceptance_context import context
rows=[]
for kind in ("identity-empty","placed-empty","reflected-nonempty","custom-axes"):
 s=Scene();h=History(s);parent=Group(name=kind);parent.component=True;parent.xform=QMatrix4x4()
 if kind!="identity-empty":parent.xform.translate(1,-1,2);parent.xform.rotate(23,V(0,0,1));parent.xform.scale(-2 if kind=="reflected-nonempty" else 2,3,1)
 if kind in ("reflected-nonempty","custom-axes"):parent.mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)])
 if kind=="custom-axes":parent.axes=QMatrix4x4();parent.axes.translate(4,-2,1);parent.axes.rotate(17,V(0,1,0))
 parent.children.append(Group(name="child"));s.groups=[parent,copy_group(parent,V(10,0,0))];s.select([parent]);initial=list(group_frame(parent).data());s.begin_group_edit(parent);entry=list(s.drawing_frame.data());s.end_group_edit();assert not h.undo_stack and not s.selection
 stored=None if parent.axes is None else list(parent.axes.data());s.begin_group_edit(parent);reentry=list(s.drawing_frame.data());axes.sync(s.drawing_frame)
 assert initial==entry==reentry
 u,v=axes.plane_axes(V(0,0,1))
 rows.append({"kind":kind,"history_after_no_edit":len(h.undo_stack),"selection_after_exit":[],"initial_drawing_frame":initial,"entry_drawing_frame":entry,"stored_axes_after_exit":stored,"reentry_drawing_frame":reentry,"origin":list(axes.origin().toTuple()),"axes":{k:list(axes.axis(k).toTuple()) for k in ("x","y","z")},"plane_z_u":list(u.toTuple()),"plane_z_v":list(v.toTuple())})
 s.end_group_edit()
axes.sync(None)
Path("docs/audits/component-reentry-source.json").write_text(json.dumps({"provenance":context(),"scope":"Actual Scene begin/end edit, History and group_frame/core.axes on reentry; no pointer/viewport inference acceptance.","cases":rows},indent=2)+"\n")
print("Original no-edit history and stored drawing-frame reentry controls PASS")
