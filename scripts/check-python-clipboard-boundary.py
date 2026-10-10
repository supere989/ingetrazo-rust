#!/usr/bin/env python3
"""Actual original reopening and affine texture oracle across edit contexts."""
import os,sys,json,tempfile,subprocess,zipfile
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.group import world_mesh
from formats import igz
def points(face):return sorted(tuple(round(v,5) for v in (p.x(),p.y(),p.z())) for p in face.vertices)
def uv(face):
 a=face.attrs["texture"]["uvw"]
 return sorted((tuple(round(x,5) for x in (p.x(),p.y(),p.z())),round(sum(a[i]*[p.x(),p.y(),p.z()][i] for i in range(3))+a[3],5),round(sum(a[i+4]*[p.x(),p.y(),p.z()][i] for i in range(3))+a[7],5)) for p in face.vertices)
def main():
 app=QApplication.instance() or QApplication([])
 with tempfile.TemporaryDirectory() as t:
  root=Path(t);subprocess.run(["cargo","run","--locked","-p","ingetrazo-gui","--example","clipboard_boundary_fixture","--",t],check=True)
  source,result=Scene(),Scene();igz.load_into(source,root/"context-source.igz");igz.load_into(result,root/"context-result.igz")
  reference=world_mesh(source.groups[0]).faces[0];pasted=world_mesh(result.groups[1]).faces[0]
  assert points(reference)==points(pasted),(points(reference),points(pasted));assert uv(reference)==uv(pasted),(uv(reference),uv(pasted))
  assert pasted.attrs["texture"]["vendor_projection"]==71
  assert points(source.groups[0].mesh.faces[0])!=points(pasted),"local-coordinate negative control"
  resources=Scene();igz.load_into(resources,root/"resources-result.igz")
  assert len(resources.groups)==2;assert resources.groups[0].mesh is resources.groups[1].mesh,"shared native prototype after reopen"
  for g in resources.groups:
   f=g.mesh.faces[0];assert f.attrs["mat"]=="Paint (clipboard 1)"
   assert f.attrs["texture"]["vendor_note"]=={"path":"same.png","material":"Paint"}
  assert resources.materials["Paint"].color==(0.,0.,1.,1.);assert resources.materials["Paint (clipboard 1)"].color==(1.,0.,0.,1.)
  with zipfile.ZipFile(root/"resources-result.igz") as archive:
   blobs=[archive.read(n) for n in archive.namelist() if n!="document.json"]
   assert bytes([9]) in blobs and bytes([1,2,3]) in blobs,"both resource payloads survive actual archive"
 report={"provenance":context(),"passed":True,"negative_controls_detected":1,"scope":"Actual original world_mesh geometry/affine UV equivalence under reflected nonuniform source and nonuniform rotated destination edit frames; atomic history, foreign component repeat sharing, preserved destination materials/assets and schema-limited opaque metadata. System transport and overlapping planes are separate acceptance."}
 Path("docs/audits/python-clipboard-boundary-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
