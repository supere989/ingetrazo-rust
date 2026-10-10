#!/usr/bin/env python3
import os,sys,json,tempfile,subprocess,math
from pathlib import Path
import numpy as np
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QMatrix4x4
from core.scene import Scene
from core.group import Group
from core.mesh import Mesh
from core.parts import part_size,part_points,part_material,cut_list,cut_list_text,is_surface
from core.units import format_length
from tests.test_parts import _box,_part
from formats import igz
def canonical_lines(lines):return [{"names":l["names"],"materials":l["materials"],"size":[round(v,6) for v in l["size"]],"qty":l["qty"]} for l in lines]
def main():
 app=QApplication.instance() or QApplication([]);errors=[];pointcases=[]
 box=lambda size:[[x,y,z] for x in (0,size[0]) for y in (0,size[1]) for z in (0,size[2])]
 points=box([.04,.04,.58]);angle=math.pi/6;r=np.array([[math.cos(angle),0,math.sin(angle)],[0,1,0],[-math.sin(angle),0,math.cos(angle)]])
 pointcases.append(np.array(points)@r.T);pointcases.append(np.array([(x,y,z) for x in (0,.442) for y in (-.052,.56) for z in (.085,.794)]+[(x,y,z) for x in (0,.442) for y in (0,.56) for z in (0,.085)]))
 pointcases.extend([np.array(box([1,1,1])),np.array(box([1,1,1]))@r.T,np.array([[0,0,0],[1,2,3]]),np.array([[0,0,0]]),np.array([]).reshape(0,3),np.array(box([1,1,0]))@r.T])
 scene=Scene()
 for name,size,material in [("Piece 10",(.5805,.04,.04),"Wood"),("Piece 2",(.5805,.04,.04),"Wood"),("Piece 1",(.5814,.04,.04),"Steel"),("Skin",(.4,.3,0),"Paint")]:
  g=_part(_box(*size),name);g.material={"mat":material};scene.groups.append(g)
 leaf=Group(_box(1,2,.1),"leaf");middle=Group(name="middle");middle.material={"mat":"Middle paint"};middle.children=[leaf];root=Group(name="Nested paint");root.material={"mat":"Root paint"};root.children=[middle];scene.groups.append(root)
 with tempfile.TemporaryDirectory() as tmp:
  directory=Path(tmp);(directory/"points.json").write_text(json.dumps([p.tolist() for p in pointcases]));igz.save_scene(scene,directory/"source.igz");subprocess.run(["cargo","run","--locked","-p","ingetrazo-gui","--example","parts_acceptance_fixture","--",str(directory)],check=True)
  result=json.loads((directory/"result.json").read_text());size_errors=[]
  for i,(points,actual) in enumerate(zip(pointcases,result["sizes"])):
   expected=part_size(points)
   if max(abs(a-b) for a,b in zip(expected,actual))>1e-5:size_errors.append({"case":i,"original":expected,"rust":actual})
  if size_errors:errors.append({"point_sizes":size_errors})
  original=Scene();igz.load_into(original,directory/"source.igz");rows=[{"name":g.name,"material":part_material(g),"size":part_size(part_points(g)),"faces":sum(len(p.mesh.faces) for p in [g]+g.children+[leaf] if p is not None),"hidden":g.hidden} for g in original.groups]
  # Count recursively, preserving original nested material fallback semantics.
  def faces(g):return len(g.mesh.faces)+sum(faces(c) for c in g.children)
  for g,row,actual in zip(original.groups,rows,result["summaries"]):
   row["faces"]=faces(g)
   expected={**row,"size":[round(v,6) for v in row["size"]]};actual={**actual,"size":[round(v,6) for v in actual["size"]]}
   if actual!=expected:errors.append({"summary":g.name,"original":expected,"rust":actual})
  for case in result["lists"]:
   subset=[r for r in rows if case["surfaces"] or not is_surface(r["size"])]
   expected=canonical_lines(cut_list(subset,case["precision"],case["by_material"]));actual=canonical_lines(case["lines"])
   if expected!=actual:errors.append({"list_options":{k:case[k] for k in ("precision","by_material","surfaces")},"original":expected,"rust":actual})
  headers=["Qty","Parts","Material","Length","Width","Thickness"];text_errors=[]
  for unit,actual in result["texts"].items():
   decimals=max(2,{"m":3,"cm":1,"mm":0,"in":2,"ft":3}[unit]);expected=cut_list_text(cut_list(rows),lambda v:format_length(v,unit,decimals),headers)
   if actual!=expected:text_errors.append({"unit":unit,"original":expected,"rust":actual})
  errors.extend(text_errors)
 report={"provenance":context(),"passed":not errors,"point_cases":len(pointcases),"list_option_cases":12,"errors":errors,"scope":"Original PCA/model-axis extent including splayed/stepped/repeated eigenvalues/degenerate/surface clouds; actual native nested paint and cut-list sizes/names/material merge/precision/surface controls, five document-unit text variants. Rendered caching/clipboard UX remains open."}
 Path("docs/audits/python-parts-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2));return 0 if report["passed"] else 1
if __name__=="__main__":raise SystemExit(main())
