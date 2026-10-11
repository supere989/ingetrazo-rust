#!/usr/bin/env python3
import sys,json,copy
from pathlib import Path
p=Path(sys.argv[1]);original=json.loads((p/'admission-original.json').read_text());native=json.loads((p/'admission-native.json').read_text())
cases=['near','outside','nested-shared','billboard','orphan'];phases=['before','create','create-undo','create-redo','move','undo','redo']
assert [(r['case'],r['phase'])for r in original['states']]==[(c,q)for c in cases for q in phases]
for r in original['states']:
 if r['dimensions']:
  assert r['dimensions'][0]['anchored']==[r['case']in ['near','nested-shared']]*2

def typed(a,b):
 assert type(a)is type(b),'JSON type differs'
 if isinstance(a,dict):
  assert a.keys()==b.keys()
  for k in a:typed(a[k],b[k])
 elif isinstance(a,list):
  assert len(a)==len(b)
  for x,y in zip(a,b):typed(x,y)
 else:assert a==b,(a,b)
def validate(x):typed(original,x)
validate(native)
for i in range(35):
 bad=copy.deepcopy(native);bad['states'][i]['undo']+=1
 try:validate(bad)
 except AssertionError:pass
 else:raise AssertionError('accepted history mutation')
for case in ['near','nested-shared','billboard','orphan']:
 bad=copy.deepcopy(native);r=next(r for r in bad['states']if r['case']==case and r['phase']=='create');r['dimensions'][0]['anchored'][0]=not r['dimensions'][0]['anchored'][0]
 try:validate(bad)
 except AssertionError:pass
 else:raise AssertionError('accepted admission mutation')
try:validate(json.loads((p/'first-native-inverse-mismatch.json').read_text()))
except AssertionError:pass
else:raise AssertionError('accepted retained wrong-inverse candidate')
print(json.dumps(dict(states=35,negatives=39,retained_wrong_inverse_rejected=1,typed_exact=True)))
