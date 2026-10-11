#!/usr/bin/env python3
"""Independent actual saved IFC reader and required pointer-workflow inventory."""
import json, hashlib, tempfile
from pathlib import Path
import ifcopenshell, ifcopenshell.validate
root=Path(__file__).resolve().parents[1]/"docs/audits/bim-workflow"
phases=["initial","parent-entry","child-entry","active","drawn","undo","redo","csv","file-menu","export-menu","ifc"]
def read(path):
 model=ifcopenshell.open(str(path));logger=ifcopenshell.validate.json_logger();ifcopenshell.validate.validate(model,logger,express_rules=True)
 assert not logger.statements,logger.statements
 elements=model.by_type("IfcElement");assert [(e.is_a(),e.Name)for e in elements]==[("IfcSlab","root")]
 e=elements[0];rings=[]
 for shell in model.by_type("IfcOpenShell")+model.by_type("IfcClosedShell"):
  for face in shell.CfsFaces:
   rings.append([[list(p.Coordinates)for p in b.Bound.Polygon]for b in face.Bounds])
 assert rings==[[[[20.,20.,0.],[22.,20.,0.],[22.,21.,0.],[20.,21.,0.]]]],rings
 assert model.by_type("IfcElementQuantity")[0].Name=="Qto_SlabBaseQuantities"
 quantities=[(q.is_a(),q.Name,q[3])for q in model.by_type("IfcPhysicalSimpleQuantity")]
 return {"rings":rings,"quantities":quantities,"elements":[(e.is_a(),e.Name)],"express_errors":len(logger.statements)}
results={}
for name in ["ui","original-ui"]:
 d=root/name;report=json.loads((d/"report.json").read_text());assert report["passed"] is True
 assert report["original_source"]=="6be29fe437a7cd992d4e079f71720821b25216b9"
 assert [p["label"]for p in report["states"]]==phases
 assert [p["state"]["context_depth"]for p in report["states"]]==[0,1,2,2,2,2,2,2,2,2,0]
 assert report["states"][-1]["state"]["tool"]=="Rectangle"
 results[name]=read(d/"delivered.ifc")
assert results["ui"]==results["original-ui"],results
negative=[]
source=(root/"ui/delivered.ifc").read_text()
with tempfile.TemporaryDirectory()as temporary:
 p=Path(temporary)/"mutation.ifc"
 for name,a,b in [("name","'root'","'wrong'"),("class","IFCSLAB(","IFCWALL("),("coordinate","20.000000","20.000001")]:
  assert a in source,(name,a);p.write_text(source.replace(a,b,1))
  try:read(p)
  except (AssertionError,RuntimeError):negative.append(name)
  else:raise AssertionError("Accepted "+name)
result={"passed":True,"source_pin":"6be29fe437a7cd992d4e079f71720821b25216b9","scope":"Actual pointer nested entry and real CSV/IFC Save dialogs; exact exported root slab only, untyped internal face tags excluded by source census","reader_version":ifcopenshell.version,"results":results,"negative_controls":negative,"file_hashes":{name:hashlib.sha256((root/name/"delivered.ifc").read_bytes()).hexdigest()for name in results}}
(root/"independent-reader.json").write_text(json.dumps(result,indent=2)+"\n");print("PASS actual IFC reader, EXPRESS, exact rings/quantities and three mutations")
