#!/usr/bin/env python3
"""Original-authored DAE imports: world surfaces distinct from editable schema."""
import os,sys,json,tempfile,subprocess,math,hashlib,shutil
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"ingetrazo")
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group,world_mesh
from georef.datum import SceneDatum
from formats import dae,igz
def ring(points):
 p=[tuple(round(x,5) for x in a) for a in points]
 return min(tuple(p[i:]+p[:i]) for i in range(len(p))) if p else ()
def signature(s):
 fs=list(s.mesh.faces)
 for g in s.groups:fs.extend(world_mesh(g).faces)
 pts=[];area=0.;colors={}
 for f in fs:
  for a,b,c in f.triangulate():
   pts.extend([a,b,c]);n=V.crossProduct(b-a,c-a).length()/2;area+=n
   key=json.dumps(f.attrs.get("color"),sort_keys=True);colors[key]=colors.get(key,0)+n
 bounds=[[round(min(getattr(p,axis)() for p in pts),5),round(max(getattr(p,axis)() for p in pts),5)] for axis in ["x","y","z"]]
 return {"area":round(area,5),"bounds":bounds,"color_area":{k:round(v,5) for k,v in sorted(colors.items())},"loose_faces":len(s.mesh.faces),"groups":len(s.groups),"world_faces":len(fs),"rings":[[list(p) for p in ring([[v.x(),v.y(),v.z()] for v in f.vertices])] for f in fs],"holes":[[[list(p) for p in ring([[v.x(),v.y(),v.z()] for v in h])] for h in f.holes] for f in fs],"georef":None if s.georef is None else [s.georef.lat,s.georef.lon,s.georef.alt,s.georef.north]}
def main():
 app=QApplication([]);exe=os.environ.get("INGETRAZO_DAE_FIXTURE","/tmp/ingetrazo-import-target/debug/examples/dae_import_acceptance_fixture");rows=[]
 with tempfile.TemporaryDirectory() as d:
  d=Path(d)
  for name in ["painted-rectangle","holed-surface","nested-reflection","georeferenced"]:
   s=Scene();m=Mesh();f=m.add_face([V(0,0,0),V(4,0,0),V(4,4,0),V(0,4,0)],[[V(1,1,0),V(3,1,0),V(3,3,0),V(1,3,0)]] if name=="holed-surface" else []);f.attrs["color"]=[.2,.4,.6]
   if name=="nested-reflection":
    child=Group(m,"Child");child.xform=QMatrix4x4();child.xform.scale(-2,3,1);parent=Group(name="Parent");parent.xform=QMatrix4x4();parent.xform.translate(10,20,30);parent.children=[child];s.groups=[parent]
   else:s.mesh=m
   if name=="georeferenced":s.georef=SceneDatum(51.5,-.1,32,17)
   inp=d/(name+".dae");out=d/(name+".igz");dae.save_dae(s,inp)
   corpus=Path("docs/audits/dae-import-corpus");corpus.mkdir(exist_ok=True);shutil.copy2(inp,corpus/inp.name)
   original=Scene();dae.load_dae(original,inp);a=signature(original)
   process=subprocess.run([exe,str(inp),str(out)],capture_output=True,text=True)
   if process.returncode:rows.append({"case":name,"original":a,"rust_error":process.stderr,"world_match":False,"editable_schema_match":False});continue
   shutil.copy2(out,corpus/out.name);rust=Scene();igz.load_into(rust,out);b=signature(rust)
   spatial=all(a[k]==b[k] for k in ["area","bounds","color_area"]);schema=a==b
   rows.append({"case":name,"original":a,"rust":b,"world_match":spatial,"editable_schema_match":schema})
 report={"provenance":context(),"native_fixture":exe,"native_fixture_sha256":hashlib.sha256(Path(exe).read_bytes()).hexdigest(),"native_source_revision":subprocess.check_output(["git","-C","/tmp/ingetrazo-imports","rev-parse","HEAD"],text=True).strip(),"cases":rows,"world_passed":all(r["world_match"] for r in rows),"schema_passed":all(r["editable_schema_match"] for r in rows),"scope":"Original authored COLLADA painted/holed/nested reflected/georeferenced documents, actual original load compared with separate-process native decode and actual original IGZ reopen. World surface area/bounds/color allocation distinct from editable loose-mesh fusion/group/georef schema. This initial corpus does not cover texture fallback, FACEME, large reference threshold or arbitrary COLLADA."}
 Path("docs/audits/python-dae-import-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
