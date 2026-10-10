#!/usr/bin/env python3
"""Original Convert Path command recipe versus modeled topology."""
import json,os,sys,tempfile,subprocess
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QVector3D
from core.scene import Scene
from core.edits import build_add_edges
from core.history import History,CompoundCommand,DeleteGeoPathsCommand
from georef.geopath import GeoPath
def xyz(p):return [p.x(),p.y(),p.z()]
def ring(r):
 p=[tuple(round(float(x),5) for x in v) for v in r];return min(tuple(p[i:]+p[:i]) for i in range(len(p)))
def faces(rows):return sorted((ring(f["vertices"]),tuple(sorted(ring(h) for h in f["holes"])),json.dumps(f["attrs"],sort_keys=True)) for f in rows)
def edges(rows):return sorted(tuple(sorted(tuple(round(x,5) for x in p) for p in e)) for e in rows)
def main():
 square=[[0,0,0],[2,0,0],[2,2,0],[0,2,0]]
 def path(points,closed=False):return {"points":points,"closed":closed}
 cases=[{"name":"closed-footprint","faces":[],"edges":[],"paths":[path(square,True)]},{"name":"two-elevations","faces":[],"edges":[],"paths":[path(square,True),path([[x,y,2] for x,y,z in square],True)]},{"name":"open-road","faces":[],"edges":[],"paths":[path([[0,0,0],[2,0,0],[2,2,0]])]},{"name":"painted-face-chord","faces":[{"vertices":square,"attrs":{"color":[1,0,0,1],"vendor":71}}],"edges":[],"paths":[path([[0,1,0],[2,1,0]])]},{"name":"crossing-loose-edges","faces":[],"edges":[[[-1,0,0],[1,0,0]]],"paths":[path([[0,-1,0],[0,1,0]])]},{"name":"bowtie","faces":[],"edges":[],"paths":[path([[0,0,0],[2,2,0],[0,2,0],[2,0,0]],True)]}]
 with tempfile.TemporaryDirectory() as t:
  a,b=Path(t)/"cases.json",Path(t)/"results.json";a.write_text(json.dumps(cases));subprocess.run(["cargo","run","--locked","-p","ingetrazo-core","--example","geopath_acceptance_fixture","--",str(a),str(b)],check=True);results=json.loads(b.read_text());rows=[];failures=[]
  for c,r in zip(cases,results):
   s=Scene()
   for f in c["faces"]:s.mesh.add_face([QVector3D(*p) for p in f["vertices"]]).attrs.update(f["attrs"])
   for e in c["edges"]:s.mesh.add_edge(QVector3D(*e[0]),QVector3D(*e[1]))
   s.geo_paths=[GeoPath([QVector3D(*p) for p in p["points"]],p["closed"]) for p in c["paths"]]
   hist=History(s);commands=[build_add_edges(s,list(p.segments()),detect_faces=True) for p in s.geo_paths];commands.append(DeleteGeoPathsCommand(s.geo_paths[:]))
   hist.execute(CompoundCommand(commands));assert hist.last_error is None,hist.last_error
   original_faces=[{"vertices":[xyz(p) for p in f.vertices],"holes":[[xyz(p) for p in h] for h in f.holes],"attrs":f.attrs} for f in s.mesh.faces];original_edges=[[xyz(e.a),xyz(e.b)] for e in s.mesh.edges];same=faces(original_faces)==faces(r["faces"]) and edges(original_edges)==edges(r["edges"]);assert r["paths_remaining"]==len(s.geo_paths)==0
   rows.append({"case":c["name"],"original_faces":len(original_faces),"rust_faces":len(r["faces"]),"original_edges":len(original_edges),"rust_edges":len(r["edges"]),"exact_geometry_attributes_match":same})
   if not same:failures.append({"case":c["name"],"original_faces":original_faces,"rust_faces":r["faces"],"original_edges":original_edges,"rust_edges":r["edges"]})
 report={"provenance":context(),"passed":not failures,"cases":rows,"failures":failures,"scope":"Actual original MainWindow Convert Path command recipe (build_add_edges detect_faces + DeleteGeoPaths CompoundCommand); six source topology cases and native invalid selected-index atomicity. Native GUI undo and geographic projection remain separate."}
 Path("docs/audits/python-geopath-conversion-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps({k:v for k,v in report.items() if k!="failures"},indent=2));assert not failures,[f["case"] for f in failures]
if __name__=="__main__":main()
