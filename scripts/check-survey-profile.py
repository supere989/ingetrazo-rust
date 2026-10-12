#!/usr/bin/env python3
"""Literal source profile samples/stats/CSV and actual saved reader pairs."""
import json,sys,copy
from pathlib import Path
p=Path(sys.argv[1]);a=json.loads((p/'original-profile.json').read_text());b=json.loads((p/'native-profile.json').read_text())
expected=['long-sloped','closed-selected','highest-gaps','hidden-survey-layer','ambiguous','no-datum','vertical-only','no-path','zero-span']
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
def profiles_equal(a,b):
 assert a.keys()==b.keys()
 for key in a:
  if key!='states':strict(a[key],b[key],'/'+key)
 assert len(a['states'])==len(b['states'])==18
 # Runtime loader epochs are local; retain literal observations and bind
 # unchanged compute/repeat pairs within each runtime independently.
 for runtime in (a,b):
  for i in range(0,18,2):
   pair=runtime['states'][i:i+2]
   assert all(type(row['version'])is int and row['version']>=0 for row in pair),'invalid runtime epoch'
   assert pair[0]['version']==pair[1]['version'],'Profile changed runtime epoch'
 for i,(x,y)in enumerate(zip(a['states'],b['states'])):
  assert x.keys()==y.keys(),('state keys',i)
  for key in x:
   if key!='version':strict(x[key],y[key],'/states/'+str(i)+'/'+key)
profiles_equal(a,b)
r=json.loads((p/'profile-reopens.json').read_text());assert [c['case']for c in r]==expected
for c in r:strict(c['original'],c['native'])
for c in a['cases']:
 if c['csv']:assert (p/(c['name']+'-original.csv')).read_bytes()==(p/(c['name']+'-native.csv')).read_bytes(),c['name']
caches=json.loads((p/'profile-cache.json').read_text());assert [c['case']for c in caches]==expected
for c in caches:assert c['decoded']==(0 if c['case']in['ambiguous','no-datum','no-path']else 1)
assert json.loads((p/'dem-boundary.json').read_text())==dict(visible_photo_first=True,hidden_photo_selects_dem=True,network_not_started=True)
assert json.loads((p/"profile-guards.json").read_text())==[dict(guard=n,unchanged=True)for n in ["budget-refusal","missing-malformed-resource","hidden-photo-dem-open","equal-version-history-refresh"]]
boundary=json.loads((p/'omitted-empty-boundary.json').read_text());assert boundary['source_writer_triangle_count_guard'] and boundary['zero_triangle_IGZ_omits_photo_mesh'] and boundary['reopened_native_no_photo_refuses_DEM']
count=0
for i,row in enumerate(b['states']):
 c=copy.deepcopy(b);c['states'][i]['version']+=1
 try:profiles_equal(a,c)
 except AssertionError:count+=1
 else:raise AssertionError('runtime epoch change accepted')
 c=copy.deepcopy(b);c['states'][i]['undo']+=1
 try:profiles_equal(a,c)
 except AssertionError:count+=1
 else:raise AssertionError('readonly history mutation accepted')
 if row['profile']and row['profile']['samples']:
  c=copy.deepcopy(b);c['states'][i]['profile']['samples'][0]['station']+=.25
  try:profiles_equal(a,c)
  except AssertionError:count+=1
  else:raise AssertionError('chainage mutation accepted')
print(json.dumps(dict(cases=9,states=18,samples=sum(len(s['profile']['samples'])for s in a['states']if s['profile']),actual_original_reopens=9,byte_exact_CSV=sum(c['csv']for c in a['cases']),negative_controls=count,readonly_epoch_pairs=9,cache_cases=9,atomic_guards=4,passed=True)))
