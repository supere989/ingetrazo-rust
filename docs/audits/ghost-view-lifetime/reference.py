#!/usr/bin/env python3
"""Pinned original global ghost switches survive mesh history and edit contexts."""
import json,sys
from pathlib import Path
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.group import Group
from core.history import History,SnapshotCompound,AddEdgeCommand
s=Scene();h=History(s);rows=[]
def record(label):
 rows.append(dict(label=label,objects=s.show_hidden_objects,geometry=s.show_hidden_geometry,depth=len(s._edit_stack)))
h.execute(SnapshotCompound([AddEdgeCommand(V(0,0,0),V(1,0,0))]))
s.show_hidden_objects=True;h.undo();record("undo")
s.show_hidden_geometry=True;h.redo();record("redo")
p=Group(name="parent");c=Group(name="child");p.children=[c];s.groups=[p]
s.begin_group_edit(p);record("parent-entry")
s.show_hidden_objects=False;s.begin_group_edit(c);record("child-entry")
s.show_hidden_geometry=False;s.end_one_group_edit();record("child-finish")
s.show_hidden_objects=True;s.end_group_edit();record("document-finish")
s.begin_group_edit(p);s.show_hidden_geometry=True;s.end_group_edit();record("final-finish")
expected=[(True,False,0),(True,True,0),(True,True,1),(False,True,2),(False,False,1),(True,False,0),(True,True,0)]
assert [(r["objects"],r["geometry"],r["depth"])for r in rows]==expected
Path(sys.argv[1]).write_text(json.dumps(rows,indent=2)+"\n")
