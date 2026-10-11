#!/usr/bin/env python3
import sys,json,copy
from pathlib import Path
p=Path(sys.argv[1]);source=json.loads((p/'preview-original.json').read_text());native=json.loads((p/'preview-native.json').read_text())
cases=['front-z','side-x','aligned','axial-x','axial-y','nested'];phases=['before','first','span','second','preview','same','cancel','commit','undo','redo']
assert [(r['case'],r['phase'])for r in source['states']]==[(c,q)for c in cases for q in phases]
for case in cases:
 rows={r['phase']:r for r in source['states']if r['case']==case}
 assert len(rows['span']['lines'])==1 and len(rows['preview']['lines'])==3
 assert rows['preview']['label'] is not None and rows['cancel']['label'] is None
 assert rows['preview']['undo']==0 and rows['preview']['redo']==0
 assert rows['commit']['undo']==1 and len(rows['commit']['dimensions'])==1
 assert rows['preview']['label']==rows['same']['label'] and rows['preview']['lines']==rows['same']['lines']
# Source hover projects but third click uses its raw off-plane world cursor.
r={x['phase']:x for x in source['states']if x['case']=='front-z'}
assert r['span']['lines'][0][1][2]==3.0
assert r['commit']['dimensions'][0]['offset'][2]!=0.0

def typed(a,b):
 assert type(a)is type(b),'JSON type differs'
 if isinstance(a,dict):
  assert a.keys()==b.keys()
  for k in a:typed(a[k],b[k])
 elif isinstance(a,list):
  assert len(a)==len(b)
  for x,y in zip(a,b):typed(x,y)
 else:assert a==b,(a,b)
def validate(x):typed(source,x)
validate(native)
try:validate(json.loads((p/'initial-native.json').read_text()))
except AssertionError:pass
else:raise AssertionError('accepted initial nested normalization failure')
for i in range(60):
 bad=copy.deepcopy(native);bad['states'][i]['lines']=['wrong']
 try:validate(bad)
 except AssertionError:pass
 else:raise AssertionError('accepted preview line mutation')
for case in cases:
 bad=copy.deepcopy(native);r=next(r for r in bad['states']if r['case']==case and r['phase']=='preview');r['label']['text']='wrong'
 try:validate(bad)
 except AssertionError:pass
 else:raise AssertionError('accepted live value mutation')
print(json.dumps(dict(states=60,negatives=66,typed_exact=True,retained_failure_rejected=True)))
