#!/usr/bin/env python3
import copy,json,sys
from pathlib import Path
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text())
required={"image-"+v for v in ["quadrants","alpha","cutout","opacity-0999","opacity-below","reversed","wireframe","hidden-line","xray","hidden-layer","overlap","opaque-behind","faded-behind","threshold-behind","below-threshold-behind","coplanar-trace","oblique","perspective","uv-asymmetric","uv-asymmetric-perspective","uv-negative-v","offscreen-clip","near-clipped","tinted-alpha"]}
def check(candidate):
 assert {r["case"]for r in s["cases"]}=={r["case"]for r in candidate["cases"]}==required
 for row in s["cases"]:
  assert not row["image_outline_ablation"]and not row["uv_mirror_ablation"]and row["projection_y_ablation"]is None
  assert "NVIDIA GeForce RTX 5090"in row["gl_info"]["renderer"]
  r=next(v for v in candidate["cases"]if v["case"]==row["case"])
  assert len(r["segments"])==len(row["outline_segments"])
  for a,b in zip(row["outline_segments"],r["segments"]):
   assert a["plane"]==b["plane"]
   assert [a["screen_a"],a["screen_b"]]==b["projected"],row["case"]
check(n)
for mode in ("point","missing","segment"):
 bad=copy.deepcopy(n)
 if mode=="point":bad["cases"][0]["segments"][0]["projected"][0][0]+=1e-9
 elif mode=="missing":bad["cases"].pop()
 else:bad["cases"][0]["segments"].pop()
 try:check(bad)
 except AssertionError:pass
 else:raise AssertionError("accepted mutation")
print(json.dumps({"cases":24,"visible_segments":sum(len(r["outline_segments"])for r in s["cases"]),"exact_projected_endpoints":True,"negative_controls":3,"scope":"Original MVP and source-visible border segments. Native camera MVP construction, hidden/rejected segment admission and actual GPU outline pixels remain separate requirements."}))
