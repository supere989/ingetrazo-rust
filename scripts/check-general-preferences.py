#!/usr/bin/env python3
import json,copy,sys
from pathlib import Path
p=Path(sys.argv[1]);s=json.loads((p/'original-preferences.json').read_text());n=json.loads((p/'native-preferences.json').read_text())
phases=['before','draft','cancel','accept','unlimited','lower-no-prune','next-command-pruned','restart-cancel']
assert [r['phase']for r in s['rows']]==phases
assert [r['undo']for r in s['rows']]==[3,3,3,3,5,5,2,0]
assert [r['arms']for r in s['rows']]==[0,0,0,1,1,2,2,0]
assert all(r['opaque']=='vendor-preserved'for r in s['rows'])
assert s['rows'][3]['undo_cap']==0 and s['rows'][7]['undo_cap']==2
def typed(a,b):
 assert type(a)is type(b)
 if isinstance(a,dict):
  assert a.keys()==b.keys()
  for k in a:typed(a[k],b[k])
 elif isinstance(a,list):
  assert len(a)==len(b)
  for x,y in zip(a,b):typed(x,y)
 else:assert a==b,(a,b)
def validate(x):typed(s,x)
validate(n)
for phase,key,value in [('draft','autosave',False),('cancel','minutes',7),('accept','undo_cap',1),('unlimited','undo',1),('lower-no-prune','undo',2),('next-command-pruned','undo',4),('restart-cancel','undo_cap',200),('accept','opaque','lost'),('draft','arms',1)]:
 b=copy.deepcopy(n);next(r for r in b['rows']if r['phase']==phase)[key]=value
 try:validate(b)
 except AssertionError:pass
 else:raise AssertionError((phase,key))
print(json.dumps(dict(states=8,negatives=9,typed_exact=True)))
