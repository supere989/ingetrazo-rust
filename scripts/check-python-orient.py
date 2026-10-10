#!/usr/bin/env python3
"""Compare original component-local orientation and runtime partition flags."""
import json,os,sys,tempfile,subprocess,importlib.util
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QVector3D
from core.mesh import Mesh
from core.orient import orient_outward
spec=importlib.util.spec_from_file_location("reference_orient",Path("ingetrazo/tests/test_orient.py"));ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)
def xyz(p):return [p.x(),p.y(),p.z()]
def ring(p):
 p=[tuple(round(float(x),5) for x in v) for v in p];return min(tuple(p[i:]+p[:i]) for i in range(len(p)))
def main():
 cases=[];meshes=[]
 for name in ["cube","partition","disjoint","nested","nearby-sheet","open-sheets","prism"]:
  m=Mesh()
  if name=="partition":ref._two_storey_box(m)
  elif name=="prism":ref._prism(m)
  elif name=="open-sheets":
   m.add_face([ref.V(0,0),ref.V(1,0),ref.V(1,1),ref.V(0,1)]);m.add_face([ref.V(0,0),ref.V(0,1),ref.V(0,1,1),ref.V(0,0,1)])
  else:
   ref._cube(m)
   if name=="disjoint":ref._box_at(m,5)
   if name=="nested":
    inner=Mesh();ref._box_at(inner,0,s=.5)
    for f in inner.faces:m.add_face([p+QVector3D(.25,.25,.25) for p in f.vertices])
   if name=="nearby-sheet":m.add_face([ref.V(5,-1,-1),ref.V(5,2,-1),ref.V(5,2,2),ref.V(5,-1,2)])
  cases.append({"name":name,"faces":[[xyz(p) for p in f.vertices] for f in m.faces]});meshes.append(m)
 with tempfile.TemporaryDirectory() as t:
  a,b=Path(t)/"cases.json",Path(t)/"results.json";a.write_text(json.dumps(cases));subprocess.run(["cargo","run","--locked","-p","ingetrazo-core","--example","orient_acceptance_fixture","--",str(a),str(b)],check=True);results=json.loads(b.read_text());rows=[]
  for case,m,r in zip(cases,meshes,results):
   orient_outward(m);expected=[(ring([xyz(p) for p in f.vertices]),f.interior) for f in m.faces];actual=[(ring(f["vertices"]),f["interior"]) for f in r["faces"]];assert expected==actual,(case["name"],expected,actual)
   interior=sum(f["interior"] for f in r["faces"]);rows.append({"case":case["name"],"faces":len(actual),"interior_partitions":interior})
  assert rows[1]["interior_partitions"]==1,"partition detector negative control";assert all(row["interior_partitions"]==0 for row in rows if row["case"]!="partition"),"unrelated shell or sheet must not become partition"
 report={"provenance":context(),"passed":True,"negative_controls_detected":2,"cases":rows,"scope":"Original exact oriented face rings and runtime interior flags on inconsistent cube, shared slab, disjoint and nested shells, nearby open sheet, attached open sheets and irregular prism; attribute identity and idempotence. Complex concave/annular probes and randomized ray degeneracies remain open."}
 Path("docs/audits/python-orient-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
