#!/usr/bin/env python3
"""Strict semantic comparison of retained original/native delivered UI states."""
import copy,json,sys
from pathlib import Path
PHASES=["initial","active","first","rearmed","second","undo","redo","off","off-undo","off-redo"]
def signatures(report):
 assert report["passed"] is True
 states=report["states"];assert [r["label"]for r in states]==PHASES
 result=[]
 for row in states:
  s=row["state"];assert isinstance(s["active"],bool)
  result.append([row["label"],s["active"],s["tag"],s["undo"],s["redo"],sorted((f["attrs"].get("ifc",{}).get("id",-1),json.dumps(f["attrs"].get("ifc"),sort_keys=True))for f in s["faces"])])
 return result
def validate(source,native):
 assert source["original"]is True and native["original"]is False
 assert source["original_source"]==native["original_source"]=="6be29fe437a7cd992d4e079f71720821b25216b9"
 assert signatures(source)==signatures(native)
def main():
 root=Path(sys.argv[1]if len(sys.argv)>1 else"docs/audits/bim-draw")
 source=json.loads((root/"original-ui/report.json").read_text());native=json.loads((root/"ui/report.json").read_text());validate(source,native)
 negatives=[]
 for name in ["empty","duplicate-phase","lost-tag","collapsed-ID","phantom-neighbor","mode-reverted"]:
  changed=copy.deepcopy(native)
  if name=="empty":changed["states"]=[]
  elif name=="duplicate-phase":changed["states"][3]=copy.deepcopy(changed["states"][2])
  elif name=="lost-tag":del changed["states"][2]["state"]["faces"][0]["attrs"]["ifc"]
  elif name=="collapsed-ID":
   for f in changed["states"][4]["state"]["faces"]:f["attrs"]["ifc"]["id"]=1
  elif name=="phantom-neighbor":changed["states"][4]["state"]["faces"].append(copy.deepcopy(changed["states"][4]["state"]["faces"][0]))
  else:changed["states"][8]["state"]["active"]=True
  try:validate(source,changed)
  except (AssertionError,KeyError):negatives.append(name)
  else:raise AssertionError("Mutation accepted: "+name)
 report={"passed":True,"phases":PHASES,"negative_controls":negatives,"scope":"IFC class/name/object IDs, face census, runtime activation and undo/redo stack counts. Different viewport sizes/pixel input quantization excluded from UI geometry equivalence; separate source command corpus compares exact oriented rings."}
 (root/"ui-comparison.json").write_text(json.dumps(report,indent=2)+"\n");print("PASS 10 actual UI phases + six rejected mutations")
if __name__=="__main__":main()
