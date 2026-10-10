#!/usr/bin/env python3
"""Actual original/native container paint controls with retained omitted-paint failure."""
import json,sys
from pathlib import Path
source,native,baseline=[json.loads(Path(p).read_text())["cases"] for p in sys.argv[1:4]]
required={"paint-default-front","paint-default-reversed","paint-face-override","paint-explicit-back","paint-child-own","paint-top-owner-not-intermediate"}
assert required <= {c["case"] for c in source}
assert len(source)==len(native)>=16
old={c["case"]:c for c in baseline};failures=0
for a,b in zip(source,native):
 assert a["case"]==b["case"] and a["center_rgba"]==b["center_rgba"],(a["case"],a["center_rgba"],b["center_rgba"])
 front=[r for r in b["paint_runs"] if r[0] in (0,1)];back=[r for r in b["paint_runs"] if r[0]==2]
 for sr,nr in [(a["cached_opacity_runs"],front),(a["cached_back_opacity_runs"],back)]:
  assert len(sr)==len(nr),(a["case"],sr,nr)
  for x,y in zip(sr,nr):assert abs(x[0]-y[1])<1e-6 and x[-1]==y[-1]
 assert b["geometry_uploads"]==1 and "additional_position_bytes: 0" in b["contour_metrics"]
 if a["case"] in required:failures+=max(abs(x-y) for x,y in zip(a["center_rgba"],old[a["case"]]["center_rgba"]))>8
assert failures>=4,"retained omitted-paint negative controls must fail"
print(f"PASS {len(source)} exact original/native controls; inherited/own/back/top-owner paint and four retained baseline failures")
