#!/usr/bin/env python3
"""Frozen operative AddEdge command vertex and merged-edge ownership controls."""
import os,sys,json
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from core.scene import Scene
from core.history import AddEdgeCommand
from PySide6.QtGui import QVector3D as V
s=Scene();c=AddEdgeCommand(V(0,0,0),V(1,0,0),soft=True,curve=7);c.do(s)
vertices=list(s.mesh.vertices);edge=c.edge;c.undo(s)
assert len(s.mesh.edges)==0 and len(s.mesh.vertices)==2
assert all(a is b for a,b in zip(vertices,s.mesh.vertices))
c.do(s);assert s.mesh.edges[0] is edge
merged=AddEdgeCommand(V(1,0,0),V(0,0,0),soft=False,curve=9);merged.do(s);merged.undo(s)
assert len(s.mesh.edges)==1 and not edge.soft and edge.curve==9
print(json.dumps({"undo_vertex_count":2,"undo_retains_vertex_objects":True,"redo_retains_edge_object":True,"merged_undo_edge_count":1,"merged_undo_soft":edge.soft,"merged_undo_curve":edge.curve},indent=2))
