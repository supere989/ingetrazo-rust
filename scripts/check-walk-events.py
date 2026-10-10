#!/usr/bin/env python3
import json,sys,math,copy
from pathlib import Path
s,n,old=[json.loads(Path(p).read_text())for p in sys.argv[1:]]
names=['split-press-motion','press-motion-one-frame','press-motion-release-one-frame','hover-control-released-before-tick']
def check(n):
 assert [r['name']for r in s['cases']]==names==[r['name']for r in n['cases']]
 for a,b in zip(s['cases'],n['cases']):
  assert a['undo']==b['undo']==0
  assert all(len(r[k])==3 for r in (a,b)for k in ('eye','forward')),'camera vectors must have three coordinates'
  assert all(math.isfinite(v)for r in (a,b)for k in ('eye','forward')for v in r[k])
  assert all(math.isfinite(y)and abs(x-y)<=1e-6 for k in ['eye','forward']for x,y in zip(a[k],b[k])),a['name']
check(n)
for index in [1,3]:
 m=copy.deepcopy(n);m['cases'][index]=old['cases'][index]
 try:check(m)
 except AssertionError:pass
 else:raise AssertionError('old anchor/modifier defect admitted')
for name,mutate in [('missing-release',lambda m:m['cases'].pop(2)),('spurious-step-after-release',lambda m:m['cases'][2]['eye'].__setitem__(1,.0198)),('history',lambda m:m['cases'][0].__setitem__('undo',1))]:
 m=copy.deepcopy(n);mutate(m)
 try:check(m)
 except AssertionError:pass
 else:raise AssertionError(name)
m=copy.deepcopy(n);m['cases'][0]['eye']=[]
try:check(m)
except AssertionError:pass
else:raise AssertionError('truncated eye accepted')
print(json.dumps(dict(passed=True,cases=4,compiled_old_rejected=2,other_negatives=4,bound='unchanged one-step Qt/native1e-6',scope='actual frozen tool contexts and timer-slot versus egui frames; wall-clock/rendered Walk remains required')))
