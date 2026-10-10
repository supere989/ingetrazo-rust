#!/usr/bin/env python3
"""Original COLLADA reference threshold, bare libraries and shared node definitions."""
import os,sys,json,tempfile,subprocess,shutil,hashlib,copy,xml.etree.ElementTree as ET,importlib.util
from pathlib import Path
from acceptance_context import context
spec=importlib.util.spec_from_file_location("dae_import",Path(__file__).with_name("check-python-dae-import.py"));oracle=importlib.util.module_from_spec(spec);spec.loader.exec_module(oracle)
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.mesh import Mesh
from formats import dae,igz
NS="{http://www.collada.org/2005/11/COLLADASchema}"
def summary(s):
 r=oracle.signature(s);r["group_names"]=[g.name for g in s.groups];r["group_faces"]=[len(g.mesh.faces)for g in s.groups];r["group_edges"]=[len(g.mesh.edges)for g in s.groups];r["components"]=[g.is_component()for g in s.groups];r["shared_pairs"]=[[i,j]for i in range(len(s.groups))for j in range(i+1,len(s.groups))if s.groups[i].mesh is s.groups[j].mesh];return r
def main():
 app=QApplication([]);exe=os.environ.get("INGETRAZO_DAE_FIXTURE")
 if exe is None:
  subprocess.run(["cargo","build","--locked","-p","ingetrazo-formats","--example","dae_import_acceptance_fixture"],check=True);exe=str(Path(os.environ.get("CARGO_TARGET_DIR","target"))/"debug/examples/dae_import_acceptance_fixture")
 rows=[];root=Path("docs/audits/dae-reference-corpus");root.mkdir(exist_ok=True)
 with tempfile.TemporaryDirectory()as d:
  d=Path(d);binary=d/"decoder";shutil.copy2(exe,binary);digest=hashlib.sha256(binary.read_bytes()).hexdigest()
  for name in ["threshold400","threshold401","structured512","bare512","shared-library512","painted512"]:
   s=Scene();m=s.mesh;w,h=(20,10)if name.startswith("threshold")else(16,16)
   for x in range(w):
    for y in range(h):
     f=m.add_face([V(x,y,0),V(x+1,y,0),V(x+1,y+1,0),V(x,y+1,0)])
     if name=="painted512":f.attrs["color"]=[1,0,0]if x%2 else[0,1,0]
   if name=="threshold401":m.add_face([V(21,0,0),V(22,0,0),V(21,1,0)])
   inp=d/(name+".dae");out=d/(name+".igz");dae.save_dae(s,inp);tree=ET.parse(inp);r=tree.getroot()
   if name=="bare512":
    for tag in ["library_visual_scenes","scene"]:
     e=r.find(NS+tag)
     if e is not None:r.remove(e)
   if name=="shared-library512":
    vs=r.find(NS+"library_visual_scenes/"+NS+"visual_scene");node=vs.find(NS+"node");library=ET.SubElement(r,NS+"library_nodes");library_node=copy.deepcopy(node);library_node.set("id","SharedGrid");library.append(library_node);vs.clear();vs.set("id","Scene")
    for i in range(2):
     parent=ET.SubElement(vs,NS+"node",id="Copy"+str(i),name="Assembly"+str(i));ET.SubElement(parent,NS+"translate").text=str(i*40)+" 0 0";ET.SubElement(parent,NS+"instance_node",url="#SharedGrid",name="Instance"+str(i))
   tree.write(inp,encoding="unicode");shutil.copy2(inp,root/inp.name);original=Scene();dae.load_dae(original,inp);a=summary(original)
   process=subprocess.run([str(binary),str(inp),str(out)],capture_output=True,text=True)
   if process.returncode:rows.append({"case":name,"original":a,"rust_error":process.stderr,"world_match":False,"editable_schema_match":False});continue
   shutil.copy2(out,root/out.name);rust=Scene();igz.load_into(rust,out);b=summary(rust)
   world=all(a[k]==b[k]for k in ["area","bounds","color_area","textures"]);same=oracle.editable(a,b);rows.append({"case":name,"original":a,"rust":b,"world_match":world,"editable_schema_match":same})
 controls=[]
 for name,index,field,value in [("lost-boundary-stations",0,"rings",[[[0,0,0],[20,0,0],[20,10,0],[0,10,0]]]),("lost-shared-definition",4,"shared_pairs",[]),("wrong-reference-name",3,"group_names",["Wrong"])]:
  original=rows[index]["original"];changed=copy.deepcopy(original);changed[field]=value;detected=not oracle.editable(original,changed);assert detected;controls.append({"case":name,"detected":True})
 original=rows[5]["original"];changed=copy.deepcopy(original);paint=json.loads(changed["surface_recipes"][0]);paint["attrs"]["color"]=[0,0,0];changed["surface_recipes"][0]=json.dumps(paint,sort_keys=True);assert not oracle.editable(original,changed);controls.append({"case":"repainted-region-same-area","detected":True})
 report={"negative_controls":controls,"provenance":context(),"native_source_revision":os.environ.get("INGETRAZO_DAE_SOURCE_REVISION",context()["rust_commit"]if"INGETRAZO_DAE_FIXTURE"not in os.environ else"unverified"),"native_fixture_sha256":digest,"passed":all(r["world_match"]and r["editable_schema_match"]for r in rows),"cases":rows,"scope":"Original actual load across 400/401 polygon threshold, structured/bare 512-triangle library, repeated instance_node prototype sharing and paint seams. World surfaces distinct from loose/reference grouping, boundary stations and shared-definition identity. Open winding uses retained original nondeterminism boundary. FACEME/resources/complex hierarchy and UI import undo remain separate."}
 Path("docs/audits/python-dae-reference-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps([{"case":r["case"],"world":r["world_match"],"schema":r["editable_schema_match"],"original_groups":r["original"]["group_names"],"rust_groups":r.get("rust",{}).get("group_names"),"original_shared":r["original"]["shared_pairs"],"rust_shared":r.get("rust",{}).get("shared_pairs")}for r in rows],indent=2))
 assert report["passed"],[r["case"]for r in rows if not r["editable_schema_match"]]
if __name__=="__main__":main()
