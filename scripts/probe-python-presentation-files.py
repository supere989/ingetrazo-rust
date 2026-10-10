#!/usr/bin/env python3
"""Actual pinned IGZ load and native-save reopen; no renderer claim."""
import copy,json,sys
from pathlib import Path
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from core.scene import Scene
from formats import igz
from acceptance_context import context
mode=sys.argv[1];root=Path(sys.argv[2]);root.mkdir(parents=True,exist_ok=True)
def read_case(i,path):
 try:
  scene=Scene();igz.load_into(scene,path)
  return {"case":i,"style":scene.display_style.to_dict(),"shadows":scene.shadows.to_dict(),"error":None}
 except (TypeError,ValueError,OverflowError) as e:
  return {"case":i,"error":type(e).__name__}
if mode=="generate":
 source=json.loads(Path(sys.argv[3]).read_text());template=json.loads(Path(sys.argv[4]).read_text())
 (root/"cases.json").write_text(json.dumps(source,indent=2)+"\n")
 rows=[]
 for i,row in enumerate(source["cases"]):
  doc=copy.deepcopy(template);doc["scene"]["display_style" if row["kind"]=="style" else "shadows"]=row["input"]
  path=root/f"case-{i}.igz";path.write_text(json.dumps(doc,indent=2)+"\n");rows.append(read_case(i,path))
 (root/"original.json").write_text(json.dumps({"provenance":context(),"cases":rows},indent=2)+"\n")
elif mode=="reopen":
 original=json.loads((root/"original.json").read_text())["cases"];rows=[]
 for row in original:
  path=root/f"native-{row["case"]}.igz"
  if row["error"] is not None:
   assert not path.exists(),"refused input was saved";continue
  rows.append(read_case(row["case"],path))
 assert rows==[r for r in original if r["error"] is None],"native-save original reopen changed recipe"
 (root/"reopened.json").write_text(json.dumps({"provenance":context(),"cases":rows},indent=2)+"\n")
 print(json.dumps({"reopened":len(rows),"rendered_claim":False}))
else:raise ValueError(mode)
