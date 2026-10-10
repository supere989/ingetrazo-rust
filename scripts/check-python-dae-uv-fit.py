#!/usr/bin/env python3
"""Actual original COLLADA polygon UV fitting versus separate native decoder."""
import os,sys,json,tempfile,subprocess,shutil,hashlib,xml.etree.ElementTree as ET,importlib.util,copy
from pathlib import Path
spec=importlib.util.spec_from_file_location("dae_oracle",Path(__file__).with_name("check-python-dae-import.py"));oracle=importlib.util.module_from_spec(spec);spec.loader.exec_module(oracle)
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QImage,QColor
from core.scene import Scene
from formats import dae,igz
NS="{http://www.collada.org/2005/11/COLLADASchema}"
app=QApplication([]);exe=os.environ["INGETRAZO_DAE_FIXTURE"];revision=os.environ["INGETRAZO_DAE_SOURCE_REVISION"];rows=[];root=Path("docs/audits/dae-uv-fit-corpus");root.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory()as d:
 d=Path(d);binary=d/"decoder";shutil.copy2(exe,binary);digest=hashlib.sha256(binary.read_bytes()).hexdigest()
 for name,points,uv in [
  ("nonaffine-quad",[[0,0,0],[4,0,0],[4,4,0],[0,4,0]],[[0,0],[1,0],[1,1],[.3,.7]]),
  ("near-collinear-quad",[[0,0,0],[.001,.00000001,0],[4,4,0],[0,4,0]],[[0,0],[.00025,.0000000025],[1,1],[0,1]]),
  ("near-collinear-first-points",[[0,0,0],[.001,0,0],[4,0,0],[4,4,0],[0,4,0]],[[0,0],[.00025,0],[1,0],[1,1],[0,1]]),
  ("nonaffine-pentagon",[[0,0,0],[.001,0,0],[4,0,0],[4,4,0],[0,4,0]],[[0,0],[.00025,0],[1,0],[1,1],[.3,.7]])]:
  scene=Scene();face=scene.mesh.add_face([V(*p)for p in points]);image=QImage(2,2,QImage.Format_RGBA8888);image.fill(QColor(203,71,29));image.save(str(d/"paint.png"));face.attrs["texture"]={"path":str(d/"paint.png"),"uvw":[.25,0,0,0,0,.25,0,0],"sw":4,"sh":4}
  inp=d/(name+".dae");out=d/(name+".igz");dae.save_dae(scene,inp);tree=ET.parse(inp);mesh=next(tree.getroot().iter(NS+"mesh"))
  for source in mesh.findall(NS+"source"):
   if source.get("id").endswith("-pos"):values=[v for p in points for v in p];stride=3
   elif source.get("id").endswith("-uv"):values=[v for p in uv for v in p];stride=2
   else:continue
   source.find(NS+"float_array").text=" ".join(map(str,values));source.find(NS+"float_array").set("count",str(len(values)));source.find(NS+"technique_common/"+NS+"accessor").set("count",str(len(points)))
  primitive=mesh.find(NS+"triangles");primitive.tag=NS+"polylist";primitive.set("count","1");ET.SubElement(primitive,NS+"vcount").text=str(len(points));primitive.find(NS+"p").text=" ".join(str(v)for i in range(len(points))for v in [i,0,i]);tree.write(inp,encoding="unicode");shutil.copy2(inp,root/inp.name);shutil.copy2(d/"paint.png",root/"paint.png")
  original=Scene();dae.load_dae(original,inp);a=oracle.signature(original);process=subprocess.run([str(binary),str(inp),str(out)],text=True,capture_output=True)
  if process.returncode:rows.append({"case":name,"original":a,"rust_error":process.stderr,"match":False});continue
  shutil.copy2(out,root/out.name);native=Scene();igz.load_into(native,out);b=oracle.signature(native);rows.append({"case":name,"original":a,"native":b,"match":oracle.editable(a,b)})
 for row in rows:
  row["fitted_recipe_match"]=set(json.dumps(json.loads(r)["attrs"],sort_keys=True)for r in row["original"]["surface_recipes"])==set(json.dumps(json.loads(r)["attrs"],sort_keys=True)for r in row.get("native",{}).get("surface_recipes",[]))
 controls=[];original=rows[0]["original"];changed=copy.deepcopy(original);changed["surface_recipes"][0]=changed["surface_recipes"][0].replace('"sw": 4.0','"sw": 40.0');assert not oracle.editable(original,changed);controls.append({"case":"wrong-texture-scale","detected":True})
 report={"source_revision":revision,"binary_sha256":digest,"passed":all(r["match"]for r in rows),"all_fitted_recipes_match":all(r["fitted_recipe_match"]for r in rows),"negative_controls":controls,"cases":rows,"scope":"Actual original nonaffine quad/pentagon and narrow first basis polygon load; exact fitted affine recipes, evaluated world UV, image bytes and editable topology. Arbitrary degenerate/selfintersecting COLLADA and missing resource colors separate."};Path("docs/audits/dae-uv-fit-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps([{k:r[k]for k in ["case","match"]}for r in rows],indent=2));assert report["passed"]
