#!/usr/bin/env python3
"""Strict finite original/native DEM Profile, literal CSV and actual-reader gate."""
import argparse,copy,json
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument("source",type=Path);p.add_argument("native",type=Path);p.add_argument("comparison",type=Path);a=p.parse_args()
source=json.loads((a.source/"original-dem.json").read_text());native=json.loads((a.native/"native-dem.json").read_text())
def literal(x,y,path=""):
 assert type(x) is type(y),f"{path}: type {type(x).__name__}/{type(y).__name__}"
 if isinstance(x,dict):
  assert x.keys()==y.keys(),f"{path}: keys"
  for k in x:literal(x[k],y[k],path+"/"+k)
 elif isinstance(x,list):
  assert len(x)==len(y),f"{path}: length"
  for i,(u,v)in enumerate(zip(x,y)):literal(u,v,path+f"/{i}")
 else:assert x==y,f"{path}: {x!r} != {y!r}"
def compare(candidate):
 literal(source["cases"],candidate["cases"],"cases")
 assert candidate["source_pin"]==source["source_pin"]
 inventory=lambda d:[(s["case"],s["phase"])for s in d["states"]]
 assert inventory(source)==inventory(candidate) and len(candidate["states"])==30
 # Source loader increments its local version; native loader starts at zero.
 # Each independently observed action sequence must nevertheless remain read-only.
 for runtime in [source,candidate]:
  for c in runtime["cases"]:
   for restart in [False,True]:
    rows=[s for s in runtime["states"]if s["case"]==c["name"] and s["phase"].startswith("reader-cache")==restart]
    versions=[s["version"]for s in rows]
    assert all(type(v)is int and v>=0 for v in versions)
    assert len(set(versions))<=1,"readonly local epoch changed"
 for old,new in zip(source["states"],candidate["states"]):
  u=copy.deepcopy(old);v=copy.deepcopy(new)
  u.pop("version");v.pop("version")
  literal(u,v,old["case"]+"/"+old["phase"])
compare(native)
csv_count=0
for s in source["states"]:
 if s["csv"]:
  name=s["csv"];assert(a.source/name).read_bytes()==(a.native/(name[:-4]+"-native.csv")).read_bytes(),name;csv_count+=1
assert csv_count==21
reopens=json.loads((a.native/"native-reopens.json").read_text())
assert [r["case"]for r in reopens]==[c["name"]for c in source["cases"]]
for r in reopens:literal(r["original"],r["native"],r["case"]+"/reader")
guards=json.loads((a.native/"dem-guards.json").read_text())
assert [g["case"]for g in guards]==[c["name"]for c in source["cases"]]
assert all(g["readonly"] is True for g in guards)
rejected=0
for index,row in enumerate(native["states"]):
 for key,value in [("version",row["version"]+1),("provider_id","wrong-provider"),("path",99)]:
  changed=copy.deepcopy(native);changed["states"][index][key]=value
  try:compare(changed)
  except AssertionError:rejected+=1
  else:raise AssertionError(f"mutation accepted {index}/{key}")
changed=copy.deepcopy(native);changed["states"].append(copy.deepcopy(changed["states"][0]))
try:compare(changed)
except AssertionError:rejected+=1
else:raise AssertionError("extra state accepted")
result=dict(cases=5,states=30,csv=csv_count,original_reader_pairs=len(reopens),rejected=rejected,
    scope="literal profile/provider/arrival/CSV and reader pairs; versions independently readonly")
a.comparison.write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result))
