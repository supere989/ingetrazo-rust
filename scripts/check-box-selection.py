#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
p=Path(sys.argv[1]);original=json.loads((p/'original.json').read_text());native=json.loads((p/'native.json').read_text())
def validate(value, expected=None):
 expected=original['cases'] if expected is None else expected
 assert len(value['cases'])==len(expected)
 for a,b in zip(expected,value['cases']):
  assert a['name']==b['name']
  assert sorted(a['selected'])==sorted(b['selected']),(a['name'],a['selected'],b['selected'])
validate(native)
controls=0
for i,row in enumerate(native['cases']):
 bad=copy.deepcopy(native)
 if row['selected']:bad['cases'][i]['selected'].pop()
 else:bad['cases'][i]['selected']=[['face',0]]
 try:validate(bad)
 except AssertionError:controls+=1
 else:raise AssertionError('mutation accepted')
report={'states':len(native['cases']),'negative_controls':controls,'exact':True}

def guide_validate(v):
 assert v=={'edited':[{'point':[7.,8.,9.],'origin':[1.,2.,3.]},{'point':[0.,3.,0.],'direction':[1.,0.,0.]}],'fresh':[{'point':[4.,5.,6.],'origin':[0.,0.,0.]}]}
g=json.loads((p/'guide-original-reopen.json').read_text());guide_validate(g)
for key,field in [('edited','origin'),('edited','point'),('fresh','origin')]:
 bad=copy.deepcopy(g);bad[key][0].pop(field)
 try:guide_validate(bad)
 except AssertionError:pass
 else:raise AssertionError('guide loss accepted')
report.update({'guide_reopens':2,'guide_negative_controls':3})

for name,count in [('missing-guides-native.json',35),('face-layer-gate-native.json',39)]:
 path=p/'failures'/name
 assert path.is_file(),'Missing retained failure control: '+name
 try:validate(json.loads(path.read_text()),original['cases'][:count])
 except AssertionError:pass
 else:raise AssertionError('retained failing candidate accepted: '+name)
report['retained_failed_candidates_rejected']=2
print(json.dumps(report))
