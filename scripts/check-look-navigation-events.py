#!/usr/bin/env python3
import json,sys,math
from pathlib import Path
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text());old=json.loads(Path(sys.argv[3]).read_text());names=['look-move-release','look-press-move-release']
def check(s,n):
 assert [r['name']for r in s['cases']]==names and [r['name']for r in n['cases']]==names
 for a,b in zip(s['cases'],n['cases']):
  diffs=[abs(a[k]-b[k])for k in ['yaw','pitch']]+[abs(x-y)for x,y in zip(a['eye'],b['eye'])]
  assert all(math.isfinite(v)and v<=2e-5 for v in diffs),'existing Qtfloat32/nativefloat64 camera bound'
  assert b['undo']==a['undo']==0
check(s,n)
for i in range(2):
 m=json.loads(json.dumps(n));m['cases'][i]=old['cases'][i]
 try:check(s,m)
 except AssertionError:pass
 else:raise AssertionError('compiled old release-burst defect accepted')
print(json.dumps(dict(passed=True,ordered_cases=2,compiled_old_negative_controls=2,scope='actual source contexts/handlers and native egui queued events, existing camera numeric2e-5; rendered gestures separate')))
