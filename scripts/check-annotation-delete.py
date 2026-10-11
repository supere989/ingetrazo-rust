#!/usr/bin/env python3
import sys,json,copy
from pathlib import Path
p=Path(sys.argv[1]);expected=json.loads((p/'original.json').read_text());actual=json.loads((p/'native.json').read_text())
def validate(x):
 assert len(x['states'])==len(expected['states'])
 for a,b in zip(expected['states'],x['states']):
  assert a==b,(a['case'],a['phase'],a,b)
validate(actual)
for i in range(len(actual['states'])):
 bad=copy.deepcopy(actual);bad['states'][i]['identity']['labels'].reverse()
 if bad==actual:bad['states'][i]['guides']=[]
 try:validate(bad)
 except AssertionError:pass
 else:raise AssertionError('mutation accepted')
print(json.dumps({'states':len(actual['states']),'negative_controls':len(actual['states']),'exact':True}))

hover_expected=json.loads((p/'hover-delete-original.json').read_text());hover_actual=json.loads((p/'hover-delete-native.json').read_text())
order=[(case,phase)for case in ['hover-label-root','hover-dimension-root','hover-label-nested','hover-dimension-nested','selection-priority']for phase in (['before','delete','undo','redo','finish','finish-undo','finish-redo']if case.endswith('nested')else ['before','delete','undo','redo'])]
assert [(r['case'],r['phase'])for r in hover_expected['states']]==order
# Recursive type identity rejects Python bool/int aliases as well as values.
def typed(a,b):
 assert type(a)is type(b)
 if isinstance(a,dict):
  assert a.keys()==b.keys()
  for k in a:typed(a[k],b[k])
 elif isinstance(a,list):
  assert len(a)==len(b)
  for x,y in zip(a,b):typed(x,y)
 else:assert a==b
def hover_validate(value):typed(hover_expected,value)
hover_validate(hover_actual)
for index in range(len(order)):
 bad=copy.deepcopy(hover_actual);bad['states'][index]['selection']=1
 if bad==hover_actual:bad['states'][index]['selection']=0
 try:hover_validate(bad)
 except AssertionError:pass
 else:raise AssertionError('accepted synthetic hover selection')
for case in ['hover-label-root','hover-dimension-root','selection-priority']:
 r=next(r for r in hover_actual['states']if r['case']==case and r['phase']=='delete')
 assert r['selection']==0 and r['undo']==1
 r=next(r for r in hover_actual['states']if r['case']==case and r['phase']=='undo')
 assert r['selection']==0
print(json.dumps({'hover_delete_states':len(order),'hover_delete_negatives':len(order),'typed_exact':True}))
