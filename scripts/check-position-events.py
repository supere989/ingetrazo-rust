#!/usr/bin/env python3
import json,sys,math
from pathlib import Path
names=['split-drag','move-release-one-frame','press-move-release-one-frame','six-pixel-click']
s,n,old=[json.loads(Path(p).read_text())for p in sys.argv[1:]]
def check(n):
 assert [r['name']for r in n['cases']]==names==[r['name']for r in s['cases']]
 for a,b in zip(s['cases'],n['cases']):
  assert b['mode']=='Some(Look)'and a['mode']=='LookAroundTool'and a['undo']==b['undo']==0
  assert all(math.isfinite(y)and abs(x-y)<=2e-5 for k in ['eye','forward']for x,y in zip(a[k],b[k])),a['name']
check(n)
for index in [1,2]:
 m=json.loads(json.dumps(n));m['cases'][index]=old['cases'][index]
 try:check(m)
 except AssertionError:pass
 else:raise AssertionError('compiled old batched placement admitted')
m=json.loads(json.dumps(n));m['cases'].pop()
try:check(m)
except AssertionError:pass
else:raise AssertionError('missing threshold control admitted')
print(json.dumps(dict(passed=True,cases=4,compiled_old_rejected=2,missing_case_rejected=True,camera_bound='existing Qt/native 2e-5, unchanged',scope='source contexts/tool event order versus native queued egui events; rendered separately')))
