#!/usr/bin/env python3
"""Literal source profile samples/stats/CSV and actual saved reader pairs."""
import json,sys,copy
from pathlib import Path
p=Path(sys.argv[1]);a=json.loads((p/'original-profile.json').read_text());b=json.loads((p/'native-profile.json').read_text())
expected=['long-sloped','closed-selected','highest-gaps','hidden-survey-layer','ambiguous','no-datum','empty-survey','no-path','zero-span']
def strict(a,b,path=''):
 if type(a)is not type(b):assert type(a)in(int,float)and type(b)in(int,float),(path,'type')
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(path,'keys')
  for k in a:strict(a[k],b[k],path+'/'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,'length')
  for i,(x,y)in enumerate(zip(a,b)):strict(x,y,path+'/'+str(i))
 else:assert a==b,(path,a,b)
assert [c['name']for c in a['cases']]==expected
assert [(s['case'],s['phase'])for s in a['states']]==[(n,phase)for n in expected for phase in ['compute','repeat']]
strict(a,b)
r=json.loads((p/'profile-reopens.json').read_text());assert [c['case']for c in r]==expected
for c in r:strict(c['original'],c['native'])
for c in a['cases']:
 if c['csv']:assert (p/(c['name']+'-original.csv')).read_bytes()==(p/(c['name']+'-native.csv')).read_bytes(),c['name']
caches=json.loads((p/'profile-cache.json').read_text());assert [c['case']for c in caches]==expected
for c in caches:assert c['decoded']==(0 if c['case']in['ambiguous','no-datum','no-path']else 1)
assert json.loads((p/'dem-boundary.json').read_text())==dict(visible_photo_first=True,hidden_photo_selects_dem=True,network_not_started=True)
assert json.loads((p/"profile-guards.json").read_text())==[dict(guard=n,unchanged=True)for n in ["budget-refusal","missing-malformed-resource","hidden-photo-dem-open","equal-version-history-refresh"]]
count=0
for i,row in enumerate(b['states']):
 c=copy.deepcopy(b);c['states'][i]['undo']+=1
 try:strict(a,c)
 except AssertionError:count+=1
 else:raise AssertionError('readonly history mutation accepted')
 if row['profile']and row['profile']['samples']:
  c=copy.deepcopy(b);c['states'][i]['profile']['samples'][0]['station']+=.25
  try:strict(a,c)
  except AssertionError:count+=1
  else:raise AssertionError('chainage mutation accepted')
print(json.dumps(dict(cases=9,states=18,samples=sum(len(s['profile']['samples'])for s in a['states']if s['profile']),actual_original_reopens=9,byte_exact_CSV=sum(c['csv']for c in a['cases']),negative_controls=count,cache_cases=9,atomic_guards=4,passed=True)))
