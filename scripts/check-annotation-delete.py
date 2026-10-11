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
