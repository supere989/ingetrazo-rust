#!/usr/bin/env python3
import json,sys,math
from pathlib import Path
names=['left-move-release','left-press-move-release','right-move-release','right-press-move-release','left-relative-move-release','right-relative-move-release','unrelated-release-retains-left','left-release-ends-with-right-held','unowned-relative-before-press']
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text());old=json.loads(Path(sys.argv[3]).read_text())
def check(s,n):
 assert [r['name']for r in s['cases']]==names and [r['name']for r in n['cases']]==names
 for a,b in zip(s['cases'],n['cases']):
  diffs=[abs(a[k]-b[k])for k in ['yaw','pitch']]+[abs(x-y)for x,y in zip(a['eye'],b['eye'])]
  assert all(math.isfinite(v)and v<=2e-5 for v in diffs),'existing sourceQtfloat/nativefloat64 camera bound'
  assert a['captured']==False and a['undo']==b['undo']==0
check(s,n)
for i in range(9):
 mutant=json.loads(json.dumps(n));mutant['cases'][i]=old['cases'][i]
 try:check(s,mutant)
 except AssertionError:pass
 else:raise AssertionError('compiled old motion/ownership defect accepted')
mutant=json.loads(json.dumps(n));mutant['cases'].pop()
try:check(s,mutant)
except AssertionError:pass
else:raise AssertionError('missing ownership control accepted')
print(json.dumps(dict(passed=True,named_cases=9,compiled_old_defects_rejected=9,missing_case_rejected=True,scope='actual source viewport QMouseEvent handlers/native egui events; XTEST separately, not hardware or fullnavigation parity')))
