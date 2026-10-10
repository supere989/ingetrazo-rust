#!/usr/bin/env python3
"""Actual original Line/Rectangle/Freehand command recipes versus native drafting."""
import os,sys,json,tempfile,subprocess,types,copy
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"ingetrazo")
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.history import History,AddFaceCommand
from core.edits import build_add_edge,build_add_edges
from tools.arc import commit_arc
def ring(p):
 p=[tuple(round(float(x),5) for x in v) for v in p];return min(tuple(p[i:]+p[:i]) for i in range(len(p)))
def faces(fs):return sorted((ring(f["vertices"]),tuple(sorted(ring(h) for h in f["holes"])),json.dumps(f["attrs"],sort_keys=True))for f in fs)
def edge(e):return tuple(sorted(tuple(round(x,5)for x in p)for p in e["points"]))
def edges(es):return sorted(edge(e)for e in es)
def curves(es):
 groups={}
 for e in es:
  if e["curve"] is not None:groups.setdefault(e["curve"],[]).append(edge(e))
 return sorted(tuple(sorted(g))for g in groups.values())
def xyz(p):return [p.x(),p.y(),p.z()]
def main():
 sq=[[0,0,0],[4,0,0],[4,4,0],[0,4,0]];paint={"color":[1,0,0],"vendor":71}
 def op(points,kind="line"):
  return {"segments":[[a,b]for a,b in zip(points,points[1:])],"explicit":points[:-1]if kind=="rectangle" else None,"curve":99 if kind=="freehand" else None}
 def case(name,fs=[],es=[],ops=[]):return {"name":name,"faces":fs,"edges":es,"ops":ops}
 f=[{"vertices":sq,"attrs":paint}]
 wire=[{"points":[a,b],"curve":None}for a,b in zip(sq,sq[1:]+sq[:1])]
 cases=[case("painted-line-chord",f,ops=[op([[0,2,0],[4,2,0]])]),case("crossing-wires",es=[{"points":[[-1,0,0],[1,0,0]],"curve":None}],ops=[op([[0,-1,0],[0,1,0]])]),case("line-bowtie",ops=[op([[0,0,0],[2,2,0],[0,2,0],[2,0,0],[0,0,0]])]),case("rectangle-band",f,ops=[op([[1,0,0],[2,0,0],[2,4,0],[1,4,0],[1,0,0]],"rectangle")]),case("rectangle-overlap",f,ops=[op([[2,2,0],[6,2,0],[6,6,0],[2,6,0],[2,2,0]],"rectangle")]),case("freehand-closed",ops=[op(sq+[sq[0]],"freehand")]),case("freehand-open-chord",f,ops=[op([[0,2,0],[2,2,0],[4,2,0]],"freehand")]),case("old-curve-global-rebuild",es=[{"points":[[x+10,y,z]for x,y,z in e["points"]],"curve":None}for e in wire]+[{"points":[[0,0,0],[1,0,0]],"curve":71},{"points":[[1,0,0],[1,1,0]],"curve":71}],ops=[op([[20,0,0],[21,0,0]])])]
 from tests.test_rectangle_across_a_face import _step
 step,h=_step();initial=[{"vertices":[xyz(p)for p in f.vertices],"attrs":f.attrs}for f in step.faces]
 cases.append(case("rectangle-3d-step-band",initial,ops=[op([[1,0,2],[2,0,2],[2,2,2],[1,2,2],[1,0,2]],"rectangle")]))
 rows=[];failures=[]
 with tempfile.TemporaryDirectory()as d:
  a,b=Path(d)/"cases.json",Path(d)/"results.json";a.write_text(json.dumps(cases));subprocess.run(["cargo","run","--locked","-p","ingetrazo-core","--example","drafting_acceptance_fixture","--",str(a),str(b)],check=True);results=json.loads(b.read_text())
  for c,r in zip(cases,results):
   s=Scene()
   for f in c["faces"]:s.mesh.add_face([V(*p)for p in f["vertices"]]).attrs.update(f["attrs"])
   for e in c["edges"]:s.mesh.add_edge(V(*e["points"][0]),V(*e["points"][1])).curve=e["curve"]
   h=History(s)
   for o in c["ops"]:
    seg=[(V(*a),V(*b))for a,b in o["segments"]]
    if o["curve"]is not None:commit_arc(types.SimpleNamespace(scene=s,history=h),[seg[0][0]]+[b for a,b in seg])
    elif o["explicit"]is not None:h.execute(build_add_edges(s,seg,detect_faces=False,extra=[AddFaceCommand([V(*p)for p in o["explicit"]])]))
    else:
     for a,b in seg:h.execute(build_add_edge(s,a,b))
    assert h.last_error is None,h.last_error
   fs=[{"vertices":[xyz(p)for p in f.vertices],"holes":[[xyz(p)for p in hole]for hole in f.holes],"attrs":f.attrs}for f in s.faces];es=[{"points":[xyz(e.a),xyz(e.b)],"curve":e.curve}for e in s.mesh.edges]
   same=faces(fs)==faces(r["faces"])and edges(es)==edges(r["edges"])and curves(es)==curves(r["edges"]);rows.append({"case":c["name"],"match":same,"original_faces":len(fs),"rust_faces":len(r["faces"])})
   if not same:failures.append({"case":c["name"],"original_faces":fs,"original_edges":es,"rust":r})
 controls=[]
 for name,index,field,value in [("lost-region",0,"faces",[]),("lost-curve-membership",5,"edges",[dict(e,curve=None)for e in results[5]["edges"]]),("lost-wire",1,"edges",[])]:
  changed=copy.deepcopy(results[index]);changed[field]=value;detected=faces(changed["faces"])!=faces(results[index]["faces"]) or edges(changed["edges"])!=edges(results[index]["edges"]) or curves(changed["edges"])!=curves(results[index]["edges"]);assert detected
  controls.append({"case":name,"detected":True})
 report={"negative_controls":controls,"provenance":context(),"passed":not failures,"cases":rows,"failures":failures,"scope":"Original actual Line single-edge, Rectangle explicit AddFace and Freehand commit_arc pipelines; exact oriented faces/holes/paint, wire topology and curve membership. Includes original global rebuild resurrection when any disconnected curve exists. Native input nonfinite refusal. Includes actual original stepped-solid band fixture; UI sampling, projection/undo controller acceptance separate."}
 Path("docs/audits/python-drafting-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(rows));assert not failures,[f["case"]for f in failures]
if __name__=="__main__":main()
