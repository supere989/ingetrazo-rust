#!/usr/bin/env python3
import os,sys,json,tempfile,subprocess
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.topology import heal_overlapping_faces
from formats import igz
def rect(x0,y0,x1,y1,z=0):return [V(x0,y0,z),V(x1,y0,z),V(x1,y1,z),V(x0,y1,z)]
def canonical_ring(points):
 points=[tuple(round(c,5) for c in (p.x(),p.y(),p.z())) for p in points]
 return min(tuple(points[i:]+points[:i]) for i in range(len(points)))
def shape(mesh):return sorted((canonical_ring(f.vertices),tuple(sorted(min(canonical_ring(h),canonical_ring(list(reversed(h)))) for h in f.holes)),f.attrs.get("mat")) for f in mesh.faces)
def main():
 app=QApplication.instance() or QApplication([]);expected={};counts={}
 with tempfile.TemporaryDirectory() as tmp:
  directory=Path(tmp)
  for name in ("mother","island","clean","nested","ring","partial","3d","flipped"):
   s=Scene();m=s.mesh
   if name=="mother":
    m.add_face(rect(0,0,8,8));m.add_face(rect(.2,.2,3.8,7.8));m.add_face(rect(4.2,.2,7.8,7.8))
   elif name=="clean":m.add_face(rect(0,0,4,4));m.add_face(rect(4,0,8,4))
   elif name=="nested":m.add_face(rect(0,0,10,10),[rect(2,2,8,8),rect(2,2,8,5)]);m.add_face(rect(2,2,8,8))
   elif name=="ring":m.add_face(rect(1,1,3,3));m.add_face(rect(0,0,4,4),[rect(1,1,3,3)])
   else:
    m.add_face(rect(0,0,10,10));points=rect(2,2,4,4);m.add_face(list(reversed(points)) if name=="flipped" else points)
    if name=="3d":m.add_face(rect(0,0,1,1,3))
   for i,f in enumerate(m.faces):f.attrs["mat"]=name+str(i)
   igz.save_scene(s,directory/(name+"-source.igz"));removed=heal_overlapping_faces(m,partial=False if name=="island" else None)
   expected[name]=shape(m);counts[name]=len(removed)
  subprocess.run(["cargo","run","--locked","-p","ingetrazo-formats","--example","heal_acceptance_fixture","--",str(directory)],check=True)
  rows=[]
  for name,reference in expected.items():
   rust=Scene();igz.load_into(rust,directory/(name+"-result.igz"));assert shape(rust.mesh)==reference,(name,shape(rust.mesh),reference)
   removed=json.loads((directory/(name+"-removed.json")).read_text());assert len(removed)==counts[name]
   assert all(f.attrs.get("mat") for f in rust.mesh.faces),"Surviving healed face lost metadata"
   rows.append({"case":name,"remaining_faces":len(rust.mesh.faces),"removed_original_faces":len(removed),"original_rings_holes_winding_equal":True})
 report={"provenance":context(),"passed":True,"cases":rows,"scope":"Original mother coverage, island partial-off, clean subdivision, nested hole cleanup, legitimate ring, flat overlap punching, nonflat gate and reversed winding; exact face rings/holes/orientation plus surviving metadata. General complex arrangement healing remains open."}
 Path("docs/audits/python-heal-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
