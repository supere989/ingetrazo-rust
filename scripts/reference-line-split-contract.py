#!/usr/bin/env python3
"""One bounded actual pinned Line planner metadata/weld and Qt skew control capture."""
import json,sys
from pathlib import Path
sys.path.insert(0,"ingetrazo")
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.history import History
from core.edits import build_add_edge
from core.topology import segment_intersection

def xyz(p):return [p.x(),p.y(),p.z()]
def state(s,old):
 vertices=list(s.mesh.vertices);return {"vertices":[xyz(v.position)for v in vertices],"edges":[{"vertices":[vertices.index(e.v0),vertices.index(e.v1)],"soft":e.soft,"hidden":e.hidden,"layer":e.layer,"curve":e.curve,"old_identity":old.index(e)if e in old else None}for e in s.mesh.edges],"faces":[[vertices.index(v)for v in f.loop]for f in s.faces]}
rows=[]
for name in ["metadata-split-weld","long-skew-accepted","long-skew-refused"]:
 s=Scene()
 if name=="metadata-split-weld":
  e=s.mesh.add_edge(V(0,-1,0),V(0,1,0));e.soft=True;e.hidden=True;e.layer="Old"
  e=s.mesh.add_edge(V(0,-1,0),V(0,0,0));e.hidden=True;e.layer="Weld"
  e=s.mesh.add_edge(V(5,0,1),V(6,0,1));e.soft=True;e.hidden=True;e.layer="Remote"
  a,b=V(-1,0,0),V(1,0,0)
 else:
  z=.0005 if name=="long-skew-accepted" else .003
  s.mesh.add_edge(V(.1234567,-2,z),V(.1234567,2,z));a,b=V(-10000,.34567891,0),V(10000,.34567891,0)
 old=list(s.mesh.edges);before=state(s,old);probe=segment_intersection(a,b,old[0].a,old[0].b);h=History(s);h.execute(build_add_edge(s,a,b));assert h.last_error is None,h.last_error
 after=state(s,old);h.undo();undo=state(s,old);h.redo();redo=state(s,old)
 rows.append({"case":name,"a":xyz(a),"b":xyz(b),"before":before,"intersection":None if probe is None else xyz(probe),"after":after,"undo":undo,"redo":redo})
Path(sys.argv[1]).write_text(json.dumps({"source_pin":"6be29fe437a7cd992d4e079f71720821b25216b9","cases":rows},indent=2)+"\n")
