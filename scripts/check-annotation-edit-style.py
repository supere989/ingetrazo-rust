#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
p=Path(sys.argv[1]);expected=json.loads((p/'original.json').read_text());observed=json.loads((p/'native.json').read_text())
def exact(a,b):
 if type(a) is not type(b):return False
 if isinstance(a,dict):return a.keys()==b.keys() and all(exact(a[k],b[k])for k in a)
 if isinstance(a,list):return len(a)==len(b) and all(exact(x,y)for x,y in zip(a,b))
 return a==b
def validate(value):
 if not isinstance(value,dict) or set(value)!={'states'} or not isinstance(value['states'],list) or len(value['states'])!=44 or not exact(value,expected):raise ValueError('Exact44 operative states differ')
validate(observed);negative=0
for i in range(44):
 altered=copy.deepcopy(observed);altered['states'][i]['undo']+=1
 try:validate(altered)
 except ValueError:negative+=1
 else:raise AssertionError('Changed history accepted')
for name in ['failed-nested-style.json','source-label-priority.json']:
 try:validate(json.loads((p/name).read_text()))
 except ValueError:negative+=1
 else:raise AssertionError('Retained failed candidate accepted')
for malformed in [dict(states=[]),dict(states=observed['states'][:-1]),dict(states=copy.deepcopy(observed['states']))]:
 if len(malformed['states'])==44:malformed['states'][0]['undo']=False
 try:validate(malformed)
 except ValueError:negative+=1
 else:raise AssertionError('Malformed corpus accepted')
reopen=json.loads((p/'reopen.json').read_text());assert len(reopen['cases'])==4
def validate_reopens(value):
 if not isinstance(value,dict) or len(value.get('cases',[]))!=4 or any(not exact(c['original'],c['native'])for c in value['cases']):raise ValueError('Original-reader reopen differs')
validate_reopens(reopen)
for field in ['style','labels','dimensions']:
 bad=copy.deepcopy(reopen);bad['cases'][0]['native'][field]=None
 try:validate_reopens(bad)
 except ValueError:negative+=1
 else:raise AssertionError('Mutated reopen accepted')
report={'states':44,'actual_original_reopens':4,'rejected_controls':negative,'result':'PASS'};(p/'check.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))
