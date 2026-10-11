#!/usr/bin/env python3
"""Exact frozen source context recipes; no numeric tolerance."""
import copy,json,sys
from pathlib import Path
PIN="6be29fe437a7cd992d4e079f71720821b25216b9"
CASES=["classic-empty-root","classic-root-id40","placed-leaf-root-id40","nested-transformed-root-id40","linked-leaf-root-id40","tagged-mother-root-id40","tagged-group-root-id40"]
PHASES=["entered","first","second","undo","redo","finished"]
IDENTITY=[1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.]
def ring(points):
 return min(tuple(tuple(p)for p in points[i:]+points[:i])for i in range(len(points)))
def mesh(faces):
 return sorted((ring(f["vertices"]),tuple(sorted(ring(h)for h in f["holes"])),json.dumps(f["attrs"],sort_keys=True))for f in faces)
def groups(items):
 return [(g["name"],mesh(g["faces"]),groups(g["children"]),g["transform"]or IDENTITY)for g in items]
def validate(source,native):
 assert source["source_pin"]==PIN
 for report in (source,native):
  assert [r["case"]for r in report["recipes"]]==CASES
  for r in report["recipes"]:assert [p["phase"]for p in r["phases"]]==PHASES
 for sr,nr in zip(source["recipes"],native["recipes"]):
  for s,n in zip(sr["phases"],nr["phases"]):
   label=(sr["case"],s["phase"])
   assert mesh(s["active"])==mesh(n["active"]),(label,"active")
   assert mesh(s["loose"])==mesh(n["loose"]),(label,"loose")
   assert groups(s["groups"])==groups(n["groups"]),(label,"groups")
   assert s["rows"]==n["rows"],(label,"rows")
   assert s["csv"]==n["csv"],(label,"csv")
def main():
 root=Path(sys.argv[1]if len(sys.argv)>1 else"docs/audits/bim-context")
 s=json.loads((root/"original.json").read_text());n=json.loads((root/"native.json").read_text());validate(s,n)
 rejected=[]
 for name in ["empty","renamed-recipe","tag-id","opaque-attrs","parked-root","quantity","placement"]:
  x=copy.deepcopy(n)
  if name=="empty":x["recipes"]=[]
  elif name=="renamed-recipe":x["recipes"][0]["case"]="substitute"
  elif name=="tag-id":x["recipes"][1]["phases"][1]["active"][0]["attrs"]["ifc"]["id"]+=1
  elif name=="opaque-attrs":x["recipes"][5]["phases"][0]["active"][0]["attrs"]["vendor"]=0
  elif name=="parked-root":x["recipes"][1]["phases"][1]["loose"]=[]
  elif name=="quantity":x["recipes"][1]["phases"][1]["rows"][0]["area"]+=2**-22
  else:x["recipes"][3]["phases"][0]["groups"][0]["transform"][12]+=2**-22
  try:validate(s,x)
  except (AssertionError,KeyError):rejected.append(name)
  else:raise AssertionError("Accepted mutation: "+name)
 (root/"comparison.json").write_text(json.dumps({"passed":True,"cases":CASES,"phases":PHASES,"exact_phases":42,"negative_controls":rejected,"scope":"Exact oriented rings, holes, all face attrs, root loose/groups, takeoff and CSV for seven pinned source recipes. Identity matrix representation normalized; no numeric tolerance. Pointer context entry, active-context IFC export and general parked entity tools remain open."},indent=2)+"\n")
 print("PASS 42 exact source phases and seven rejected mutations")
if __name__=="__main__":main()
