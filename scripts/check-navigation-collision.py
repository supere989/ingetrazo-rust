#!/usr/bin/env python3
import sys,json
from pathlib import Path
names=['loose-face','ordinary-group','billboard-root','hidden-root','hidden-nested-visible-grandchild','billboard-nested-visible-grandchild','billboard-nested-own-face','billboard-root-visible-child']
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text());old=json.loads(Path(sys.argv[3]).read_text())
def check(s,n):
 assert [r['name']for r in s['cases']]==names and [r['name']for r in n['cases']]==names
 for a,b in zip(s['cases'],n['cases']):
  assert a['distance']==b['distance'],(a,b)
  assert b['fallback_distance']==a['distance'],'cache refusal changed collision policy'
  assert b['tiny_admission_refused']==(b['triangles']>0),'small-budget admission'
check(s,n)
for name in ['billboard-root','billboard-nested-own-face']:
 mutant=json.loads(json.dumps(n));idx=names.index(name);mutant['cases'][idx]['distance']=old['cases'][idx]['distance']
 try:check(s,mutant)
 except AssertionError:pass
 else:raise AssertionError('compiled old billboard defect accepted')
mutant=json.loads(json.dumps(n));mutant['cases'].pop()
try:check(s,mutant)
except AssertionError:pass
else:raise AssertionError('missing mandatory child control accepted')
print(json.dumps(dict(passed=True,mandatory_cases=8,compiled_old_defects_rejected=2,missing_case_rejected=True,index_and_direct_fallback_exact=True)))
