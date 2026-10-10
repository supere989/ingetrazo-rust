#!/usr/bin/env python3
"""Strict independent actual Qt coverage comparison and mutation rejection."""
import copy,json,sys
from pathlib import Path
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text())
required={"horizontal","horizontal-reversed","vertical","vertical-reversed","diagonal","diagonal-reversed","negative-slope","negative-slope-reversed","short","fractional"}
required|={"clip-left","clip-left-reversed","clip-right","clip-bottom","clip-corner","clip-outside","clip-short"}
def check(candidate):
 assert {r["name"]for r in s["cases"]}=={r["name"]for r in candidate["cases"]}==required
 for row in s["cases"]:
  native=next(r for r in candidate["cases"]if r["name"]==row["name"])
  p={(x,y):v for x,y,v in row["pixels"]}
  q={(x+i,y):v for x,y,length,v in native["spans"]for i in range(length)}
  assert p==q,row["name"]
check(n)
for mutate in ("coverage","missing","phase"):
 bad=copy.deepcopy(n)
 if mutate=="coverage":bad["cases"][0]["spans"][0][3]^=1
 elif mutate=="missing":bad["cases"].pop()
 else:
  for span in bad["cases"][0]["spans"]:span[0]+=1
 try:check(bad)
 except AssertionError:pass
 else:raise AssertionError("mutation accepted: "+mutate)
print(json.dumps({"cases":len(required),"coverage_exact":True,"negative_controls":3,"scope":"Independent device-clipped width-one antialiased DashLine. Front clipping, selected width-two outlines, native overlay and Metal remain open."}))
