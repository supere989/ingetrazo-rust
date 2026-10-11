#!/usr/bin/env python3
"""Compare retained or fresh original/native solar states with exact equality."""
import argparse,json,copy
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument("directory",type=Path);a=p.parse_args()
expected=json.loads((a.directory/"original.json").read_text());native=json.loads((a.directory/"native.json").read_text());cases=json.loads((a.directory/"input.json").read_text())
def check(value):
 assert len(value)==len(expected)==len(cases)
 for c,e,n in zip(cases,expected,value):
  assert e.keys()==n.keys(),c["name"]
  for key in e:assert e[key]==n[key],(c["name"],key,e[key],n[key])
check(native)
mutations=[]
for key,bad in [("offset",100),("utc",None),("direction",[0,0,0]),("daylight",None),("scene_direction",[0,0,0]),("position",dict(elevation=0.,azimuth=0.))]:
 changed=copy.deepcopy(native);index=next(i for i,row in enumerate(changed) if row[key]!=bad);changed[index][key]=bad
 try:check(changed)
 except AssertionError:mutations.append(key)
 else:raise AssertionError(f"Accepted corrupted {key}")
print(json.dumps(dict(cases=len(cases),exact=True,rejected=mutations)))
