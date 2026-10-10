#!/usr/bin/env python3
"""Exact original simplification geometry/attributes; retain curve drift failures."""
import json,os,sys,tempfile,subprocess,math,importlib.util
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QVector3D
from core.mesh import Mesh
from formats.fuse import simplify_mesh
spec=importlib.util.spec_from_file_location("reference_simplify",Path("ingetrazo/tests/test_simplify_mesh.py"));ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)
def xyz(p):return [p.x(),p.y(),p.z()]
def serialized(m):return [{"vertices":[xyz(p) for p in f.vertices],"holes":[[xyz(p) for p in h] for h in f.holes],"attrs":f.attrs,"interior":f.interior} for f in m.faces]
def ring(p):
 p=[tuple(round(float(x),5) for x in v) for v in p];return min(tuple(p[i:]+p[:i]) for i in range(len(p)))
def normalize(value):
 if isinstance(value,float):return round(value,5)
 if isinstance(value,list):return [normalize(v) for v in value]
 if isinstance(value,dict):return {k:normalize(v) for k,v in value.items()}
 return value
def faces(rows):return sorted((ring(f["vertices"]),tuple(sorted(ring(h) for h in f["holes"])),json.dumps(normalize(f["attrs"]),sort_keys=True),f["interior"]) for f in rows)
def main():
 cases=[];meshes=[]
 def add(name,m,angle=0,principal=False):cases.append(dict(name=name,faces=serialized(m),angle=angle,principal=principal));meshes.append(m)
 for plane in ["XY","XZ","YZ","tilted"]:add("grid-"+plane,ref._grid(plane))
 add("principal-tilted",ref._grid("tilted"),principal=True)
 for angle in [0,1,5]:
  m=Mesh()
  for i in range(90):
   a,b=math.pi*i/180,math.pi*(i+1)/180;p=[QVector3D(10*math.cos(a),10*math.sin(a),0),QVector3D(10*math.cos(b),10*math.sin(b),0),QVector3D(10*math.cos(b),10*math.sin(b),2),QVector3D(10*math.cos(a),10*math.sin(a),2)]
   m.add_face(p[:3]);m.add_face([p[0],p[2],p[3]])
  add("curve-"+str(angle),m,angle)
 for name in ["paint-seam","uv-near","uv-seam","holed-interior","tjunction"]:
  m=Mesh();p=[QVector3D(0,0,0),QVector3D(2,0,0),QVector3D(2,2,0),QVector3D(0,2,0)]
  if name=="holed-interior":
   f=m.add_face(p,[[QVector3D(.5,.5,0),QVector3D(1.5,.5,0),QVector3D(1.5,1.5,0),QVector3D(.5,1.5,0)]]);f.interior=True;f.attrs={"color":[.2,.4,.6,1],"vendor":71}
  elif name=="tjunction":
   m.add_face(p);m.add_face([p[1],QVector3D(3,0,0),QVector3D(3,1,0),QVector3D(2,1,0)]);m.add_face([QVector3D(2,1,0),QVector3D(3,1,0),QVector3D(3,2,0),p[2]])
  else:
   a,b=m.add_face(p[:3]),m.add_face([p[0],p[2],p[3]])
   if name=="paint-seam":a.attrs={"layer":"one","color":[1,0,0,1]};b.attrs={"layer":"two","color":[0,0,1,1]}
   else:
    a.attrs={"texture":{"path":"texture.png","uvw":[1,0,0,3,0,1,0,4],"vendor":71}}
    b.attrs={"texture":{"path":"texture.png","uvw":[1,0,0,3+(0.01 if name=="uv-near" else .3),0,1,0,4],"vendor":71}}
  add(name,m)
 with tempfile.TemporaryDirectory() as t:
  a,b=Path(t)/"cases.json",Path(t)/"results.json";a.write_text(json.dumps(cases));subprocess.run(["cargo","run","--locked","-p","ingetrazo-core","--example","simplify_acceptance_fixture","--",str(a),str(b)],check=True);results=json.loads(b.read_text());rows=[];errors=[]
  for c,m,r in zip(cases,meshes,results):
   removed=simplify_mesh(m,c["angle"],c["principal"]);expected=serialized(m);same=faces(expected)==faces(r["faces"]);row={"case":c["name"],"original_removed":removed,"rust_removed":r["removed"],"original_faces":len(expected),"rust_faces":len(r["faces"]),"exact_geometry_attributes_match":same};rows.append(row)
   if not same or removed!=r["removed"]:errors.append({"case":c["name"],"original":expected,"rust":r["faces"]})
 report={"provenance":context(),"passed":not errors,"cases":rows,"failures":errors,"scope":"Original exact oriented rings, holes, attrs and transient interior flags across four grids, principal-plane restriction, curved root-normal drift, paint/UV seams, protected holed partition and T junction. Near-threshold equivalent simplifications are retained as literal parity failures."}
 Path("docs/audits/python-simplify-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps({k:v for k,v in report.items() if k!="failures"},indent=2));assert not errors,[e["case"] for e in errors]
if __name__=="__main__":main()
