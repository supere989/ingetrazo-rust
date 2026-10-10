#!/usr/bin/env python3
"""Keep-base Ctrl source success and known invalid repeated-stack boundary."""
import os,sys,json,tempfile,subprocess
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"ingetrazo")
from PySide6.QtGui import QVector3D as V
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.solids import solid_report
from tools.pushpull import PushPullTool
from tests.test_pushpull_subtractive import _push_real,_StubViewport
def ring(v):
 p=[tuple(round(x,5) for x in a) for a in v];return min(tuple(p[i:]+p[:i]) for i in range(len(p)))
def faces(rows):return sorted((ring(f["vertices"]),f["interior"]) for f in rows)
def rows(s):return [{"vertices":[[p.x(),p.y(),p.z()] for p in f.vertices],"interior":f.interior} for f in s.faces]
def ctrl(s,z):
 f=next(f for f in s.faces if all(abs(v.z()-z)<1e-6 for v in f.vertices));t=PushPullTool();t.base_face=f;t.extrusion=2;t.dragging=True;t._anchor=f.centroid();t._normal=f.normal();t._attached,t._prism_cap=t._classify_base(s);t._cap_positions=t._cap_loop_positions(f);t._keep_base=True;t._commit(_StubViewport(s))
def main():
 app=QApplication([]);out=[]
 with tempfile.TemporaryDirectory() as d:
  path=Path(d)/"results.json";subprocess.run(["cargo","run","--locked","-p","ingetrazo-tools","--example","ctrl_pushpull_acceptance_fixture","--",str(path)],check=True)
  for r in json.loads(path.read_text()):
   s=Scene();f=s.mesh.add_face([V(0,0,0),V(1,0,0),V(1,1,0),V(0,1,0)]);_push_real(s,f,3)
   if r["mixed"]:
    s.mesh.add_face([V(10,0,0),V(11,0,0),V(11,1,0),V(10,1,0)])
    s.mesh.add_edge(V(20,0,0),V(21,0,0))
   ctrl(s,3);first=rows(s);match=faces(first)==faces(r["first"]);assert r["second_atomic"]
   if not r["mixed"]:assert match
   else:assert not match and len(first)==11 and len(r["first"])==12
   if r["mixed"]:assert r["sheet_retained"] and r["wire_retained"]
   ctrl(s,5);second=rows(s);body=Scene()
   for f in s.faces:
    if all(p.x()<2 for p in f.vertices):body.mesh.add_face(f.vertices)
   valid,volume=solid_report(body.mesh);assert not valid and volume is None
   out.append({"mixed":r["mixed"],"first_exact_geometry_interior":match,"original_first":first,"rust_first":r["first"],"original_second_faces":len(second),"original_second_interior":sum(f["interior"] for f in second),"original_second_solid":valid,"rust_second_refused_atomically":r["second_atomic"],"original_second":second})
 report={"provenance":context(),"passed":True,"cases":out,"scope":"Unmixed first Ctrl +2 extrusion on height3 body matches original exact oriented faces/interior. Mixed original with an unrelated coplanar sheet deletes the body bottom and clears its partition flag; Rust keeps the valid body and unrelated sheet/wire, an intentional correction. Original second +2 produces invalid partition classification; Rust explicit atomic refusal is intentional divergence. Partial caps, holes and through-depth remain open."}
 Path("docs/audits/python-ctrl-pushpull-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps({"passed":True,"cases":len(out)}))
if __name__=="__main__":main()
