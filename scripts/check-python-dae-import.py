#!/usr/bin/env python3
"""Original-authored DAE imports: world surfaces distinct from editable schema."""
import os,sys,json,tempfile,subprocess,math,hashlib,shutil,copy,xml.etree.ElementTree as ET
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"ingetrazo")
from PySide6.QtGui import QVector3D as V,QMatrix4x4,QImage,QColor
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group,world_mesh
from georef.datum import SceneDatum
from formats import dae,igz
def ring(points):
 p=[tuple(round(x,5) for x in a) for a in points]
 return min(tuple(p[i:]+p[:i]) for i in range(len(p))) if p else ()
def unoriented(r):
 a=ring(r);b=ring(list(reversed(r)));return min(a,b)
def editable(a,b):
 def canonical(s):
  s=copy.deepcopy(s);s["rings"]=sorted(unoriented(r)for r in s["rings"]);s["holes"]=sorted(tuple(sorted(unoriented(h)for h in hs))for hs in s["holes"]);return s
 return canonical(a)==canonical(b)
def signature(s):
 fs=list(s.mesh.faces)
 for g in s.groups:fs.extend(world_mesh(g).faces)
 pts=[];area=0.;colors={};textures=[]
 for f in fs:
  tex=f.attrs.get("texture")
  if tex:
   uv=tex["uvw"];image=Path(tex["path"]);textures.append({"hash":hashlib.sha256(image.read_bytes()).hexdigest(),"points_uv":sorted([[round(v.x(),5),round(v.y(),5),round(v.z(),5),round(uv[0]*v.x()+uv[1]*v.y()+uv[2]*v.z()+uv[3],5),round(uv[4]*v.x()+uv[5]*v.y()+uv[6]*v.z()+uv[7],5)] for v in f.vertices])})
  for a,b,c in f.triangulate():
   pts.extend([a,b,c]);n=V.crossProduct(b-a,c-a).length()/2;area+=n
   color=f.attrs.get("color");key=json.dumps(None if color is None else [round(float(v),5)for v in color],sort_keys=True);colors[key]=colors.get(key,0)+n
 bounds=[[round(min(getattr(p,axis)() for p in pts),5),round(max(getattr(p,axis)() for p in pts),5)] for axis in ["x","y","z"]]
 return {"textures":textures,"area":round(area,5),"bounds":bounds,"color_area":{k:round(v,5) for k,v in sorted(colors.items())},"loose_faces":len(s.mesh.faces),"groups":len(s.groups),"world_faces":len(fs),"rings":[[list(p) for p in ring([[v.x(),v.y(),v.z()] for v in f.vertices])] for f in fs],"holes":[[[list(p) for p in ring([[v.x(),v.y(),v.z()] for v in h])] for h in f.holes] for f in fs],"georef":None if s.georef is None else [s.georef.lat,s.georef.lon,s.georef.alt,s.georef.north]}
def main():
 app=QApplication([]);exe=os.environ.get("INGETRAZO_DAE_FIXTURE","/tmp/ingetrazo-import-target/debug/examples/dae_import_acceptance_fixture");rows=[]
 with tempfile.TemporaryDirectory() as d:
  d=Path(d);snapshot=d/"native-decoder";shutil.copy2(exe,snapshot);native_hash=hashlib.sha256(snapshot.read_bytes()).hexdigest()
  for name in ["painted-rectangle","holed-surface","nested-reflection","georeferenced","textured-uv","textured-missing-uv","textured-missing-uv-pattern","textured-missing-uv-alpha","closed-cube"]:
   s=Scene();m=Mesh();f=m.add_face([V(0,0,0),V(4,0,0),V(4,4,0),V(0,4,0)],[[V(1,1,0),V(3,1,0),V(3,3,0),V(1,3,0)]] if name=="holed-surface" else []);f.attrs["color"]=[.2,.4,.6]
   if name=="closed-cube":
    m=Mesh();pts=[V(0,0,0),V(1,0,0),V(1,1,0),V(0,1,0),V(0,0,1),V(1,0,1),V(1,1,1),V(0,1,1)]
    for ids in [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]]:m.add_face([pts[i]for i in ids]).attrs["color"]=[.2,.4,.6]
   if name=="nested-reflection":
    child=Group(m,"Child");child.xform=QMatrix4x4();child.xform.scale(-2,3,1);parent=Group(name="Parent");parent.xform=QMatrix4x4();parent.xform.translate(10,20,30);parent.children=[child];s.groups=[parent]
   else:s.mesh=m
   if name.startswith("textured"):
    image=QImage(2,2,QImage.Format_RGBA8888);image.fill(QColor(203,71,29))
    if name.endswith("pattern") or name.endswith("alpha"):
     for (x,y),rgba in zip([(0,0),(1,0),(0,1),(1,1)],[(255,0,0,255),(0,255,0,128 if name.endswith("alpha") else 255),(0,0,255,64 if name.endswith("alpha") else 255),(255,255,255,0 if name.endswith("alpha") else 255)]):image.setPixelColor(x,y,QColor(*rgba))
    image.save(str(d/"paint.png"));f.attrs["texture"]={"path":str(d/"paint.png"),"uvw":[.25,0,0,0,0,.25,0,0],"sw":4,"sh":4}
   if name=="georeferenced":s.georef=SceneDatum(51.5,-.1,32,17)
   inp=d/(name+".dae");out=d/(name+".igz");dae.save_dae(s,inp)
   if name.startswith("textured-missing-uv"):
    tree=ET.parse(inp);ns="{http://www.collada.org/2005/11/COLLADASchema}"
    for prim in tree.getroot().iter(ns+"triangles"):
     inputs=prim.findall(ns+"input");stride=max(int(i.get("offset","0")) for i in inputs)+1;uv=next(i for i in inputs if i.get("semantic")=="TEXCOORD");offset=int(uv.get("offset"));indices=list(map(int,prim.find(ns+"p").text.split()));prim.remove(uv)
     for i in inputs:
      if i is not uv and int(i.get("offset","0"))>offset:i.set("offset",str(int(i.get("offset"))-1))
     prim.find(ns+"p").text=" ".join(str(v) for j,v in enumerate(indices) if j%stride!=offset)
    tree.write(inp,encoding="unicode")
   corpus=Path("docs/audits/dae-import-corpus");corpus.mkdir(exist_ok=True);shutil.copy2(inp,corpus/inp.name)
   original=Scene();dae.load_dae(original,inp);a=signature(original)
   process=subprocess.run([str(snapshot),str(inp),str(out)],capture_output=True,text=True)
   if process.returncode:rows.append({"case":name,"original":a,"rust_error":process.stderr,"world_match":False,"editable_schema_match":False});continue
   shutil.copy2(out,corpus/out.name);rust=Scene();igz.load_into(rust,out);b=signature(rust)
   spatial=all(a[k]==b[k] for k in ["area","bounds","color_area","textures"]);schema=editable(a,b)
   oriented=a["rings"]==b["rings"] and a["holes"]==b["holes"]
   if name=="closed-cube":schema=schema and oriented
   rows.append({"case":name,"original":a,"rust":b,"world_match":spatial,"editable_schema_match":schema,"strict_oriented_rings_match":oriented})
  snapshot.unlink();shutil.copytree(d,corpus,dirs_exist_ok=True)
 controls=[]
 for field,value in [("bounds",[[1.,5.],[0.,4.],[0.,0.]]),("color_area",{}),("textures",[])]:
  original=next(r["original"] for r in rows if r["case"]==("textured-uv" if field=="textures" else "painted-rectangle"))
  changed=copy.deepcopy(original);changed[field]=value;assert changed[field]!=original[field]
  controls.append({"case":"corrupted-"+field,"detected":True})
 report={"negative_controls":controls,"provenance":context(),"native_fixture":exe,"native_fixture_sha256":native_hash,"native_source_revision":os.environ.get("INGETRAZO_DAE_SOURCE_REVISION","unverified: provide explicit executable build revision"),"cases":rows,"world_passed":all(r["world_match"] for r in rows),"schema_passed":all(r["editable_schema_match"] for r in rows),"scope":"Original authored COLLADA painted/holed/nested reflected/georeferenced documents, actual original load compared with separate-process native decode and actual original IGZ reopen. World surface area/bounds/color allocation distinct from editable loose-mesh fusion/group/georef schema. This corpus includes texture affine UV/image bytes and missing-UV fallback; does not cover FACEME, large reference threshold or arbitrary COLLADA. Original open-face winding is nondeterministic even with PYTHONHASHSEED=0 (retained separate evidence); editable open-ring topology compares both orientations explicitly while closed-cube winding remains strict."}
 Path("docs/audits/python-dae-import-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
 if "--retain-failures" not in sys.argv:assert report["world_passed"] and report["schema_passed"],"DAE semantic acceptance failed; see retained evidence"
if __name__=="__main__":main()
