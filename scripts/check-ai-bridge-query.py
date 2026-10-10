#!/usr/bin/env python3
"""Strict full comparison plus explicitly bounded query/history evidence.
The retained leaf Undo orphan-lifetime defect is never converted to a PASS.
"""
import copy,json,sys
from pathlib import Path
NAMES=["empty","loose-hidden-wire-and-hole","loose-attribute-visibility","classic-and-hidden-placement","hidden-nested-parent","leaf-context","nested-context","fractional-placement","root-history","leaf-history"]
def validate(a,b,names=NAMES):
 for d in (a,b):
  assert [c["name"]for c in d["cases"]]==names,"case inventory"
  for c in d["cases"]:assert len(c["states"])==len(c["steps"])+1,"state inventory"
 assert a==b,"source/native query states mismatch"
def main():
 p=Path(sys.argv[1]);a=json.loads((p/"original.json").read_text());b=json.loads((p/"native.json").read_text());equal=a==b
 if equal:validate(a,b)
 for d in (a,b):assert [c["name"]for c in d["cases"]]==NAMES,"full source inventory"
 mismatches=[]
 for x,y in zip(a["cases"],b["cases"]):
  for index,(s,t)in enumerate(zip(x["states"],y["states"])):
   if s!=t:mismatches.append({"case":x["name"],"state":index,"fields":{k:{"original":s[k],"native":t.get(k)}for k in s if s[k]!=t.get(k)}})
 # The supported query/traversal and root-history corpus has a named inventory;
 # the separate full comparison above remains false if the leaf defect exists.
 aa=copy.deepcopy(a);bb=copy.deepcopy(b);aa["cases"]=aa["cases"][:-1];bb["cases"]=bb["cases"][:-1];validate(aa,bb,NAMES[:-1]);negative=[]
 for name in ["local-groups-only","hidden-edge-excluded","wrong-instance-count","wrong-history-outcome","missing-case"]:
  x=copy.deepcopy(bb)
  if name=="local-groups-only":x["cases"][5]["states"][1]["group_names"]=[]
  elif name=="hidden-edge-excluded":x["cases"][1]["states"][0]["bounds"]=None
  elif name=="wrong-instance-count":x["cases"][5]["states"][1]["component_instances"]=2
  elif name=="wrong-history-outcome":x["cases"][8]["history"][3]["result"]["ok"]=True
  else:x["cases"].pop()
  try:validate(aa,x,NAMES[:-1])
  except AssertionError:negative.append(name)
  else:raise AssertionError("negative accepted: "+name)
 full_negative=[]
 if equal:
  for name in ["leaf-undo-wrong-state","leaf-state-missing","leaf-step-missing","leaf-case-missing"]:
   x=copy.deepcopy(b)
   if name=="leaf-undo-wrong-state":x["cases"][-1]["states"][3]["group_names"]=["wrong leaf undo"]
   elif name=="leaf-state-missing":x["cases"][-1]["states"].pop()
   elif name=="leaf-step-missing":x["cases"][-1]["steps"].pop()
   else:x["cases"].pop()
   try:validate(a,x)
   except AssertionError:full_negative.append(name)
   else:raise AssertionError("full negative accepted: "+name)
 print(json.dumps({"full_equal":equal,"bounded_equal":True,"bounded_cases":9,"bounded_states":sum(len(c["states"])for c in aa["cases"]),"full_mismatches":mismatches,"full_negative_rejections":full_negative,"negative_rejections":negative},indent=2))
 if "--require-full"in sys.argv and not equal:raise SystemExit(1)
if __name__=="__main__":main()
