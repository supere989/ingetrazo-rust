#!/usr/bin/env python3
"""Exact source actual UI/History inventory, native datum/points and reader pairs."""
import json,sys,copy
from pathlib import Path
p=Path(sys.argv[1]);a=json.loads((p/'original-survey.json').read_text());b=json.loads((p/'native-survey.json').read_text())
def strict(a,b,path=''):
 if type(a)is not type(b):assert type(a)in(int,float)and type(b)in(int,float),(path,'type')
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(path,'keys')
  for k in a:strict(a[k],b[k],path+'/'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,'length')
  for i,(x,y)in enumerate(zip(a,b)):strict(x,y,path+'/'+str(i))
 else:assert a==b,(path,a,b)
assert [c['name']for c in a['cases']]==['fresh','existing','nested'];assert len(a['states'])==46
strict(a,b)
c=json.loads((p/'original-survey-context.json').read_text());d=json.loads((p/'native-survey-context.json').read_text());assert [s['phase']for s in c['states']]==['before','import','enter','undo','redo','finish','finish-undo','finish-redo'];strict(c,d)
r=json.loads((p/'survey-reopens.json').read_text());assert [x['case']for x in r]==['fresh','existing','nested','ancestor-history']
for row in r:strict(row['original'],row['native'])
g=json.loads((p/'survey-guards.json').read_text());assert g==dict(nonfinite_refusal_atomic=True,native_f64_geometry_untouched=True,unknown_document_untouched=True,live_camera_after_import_survives_finish=True,native_cancel_keeps_prior_camera_contract=True,later_child_entry_inherits_live_survey_camera=True)
count=0
for original,candidate in [(a,b),(c,d)]:
 for i,row in enumerate(candidate['states']):
  changed=copy.deepcopy(candidate);changed['states'][i]['undo']+=1
  try:strict(original,changed)
  except AssertionError:count+=1
  else:raise AssertionError('history mutation accepted')
  if row['points']:
   changed=copy.deepcopy(candidate);changed['states'][i]['points'][0]['position'][2]+=1
   try:strict(original,changed)
   except AssertionError:count+=1
   else:raise AssertionError('point mutation accepted')
for original,filename in [(a,'first-native-survey.json'),(c,'first-native-survey-context.json')]:
 candidate=json.loads((p/filename).read_text())
 try:strict(original,candidate)
 except AssertionError:pass
 else:raise AssertionError('retained initial Finish failure accepted')
print(json.dumps(dict(retained_failures_rejected=2,states=54,actual_original_reopens=4,negative_controls=count,guards=6,passed=True)))
