#!/usr/bin/env python3
"""Typed Select hover identity/order plus each target's actual foreground ink."""
import copy,json,sys
from pathlib import Path
p=Path(sys.argv[1]);source=json.loads((p/'original-hover.json').read_text());native=json.loads((p/'native-hover.json').read_text())
cases=['glyph','dimension','leader','edge-priority','group-priority','hidden','locked','nested']
def phases(case):return ['before','hover','same','empty','retire']+(['clear-selection']if case in ['glyph','dimension']else [])
expected={'glyph':['label',0],'dimension':['dimension',0],'leader':['label',0],'edge-priority':None,'group-priority':None,'hidden':None,'locked':None,'nested':['label',0]}
assert [(r['case'],r['phase'])for r in source['states']]==[(c,q)for c in cases for q in phases(c)]
for row in source['states']:
 target=expected[row['case']]if row['phase']in ['hover','same']else None
 assert row['annotation_hover']==target,'source inventory failed to isolate intended pick'
 selected=row['case']in ['glyph','dimension']and row['phase']!='clear-selection'
 default=[11,22,33]if row['case']in ['glyph','dimension']else [45,55,75]
 assert row['selection']==int(selected)
 assert row['inks']=={'label':None if row['case']=='hidden' else [243,115,41]if target==['label',0]or(selected and row['case']=='dimension')else default,'dimension':[243,115,41]if target==['dimension',0]or(selected and row['case']=='glyph')else default}
 assert row['same_identity'] is True

def typed_equal(a,b):
 assert type(a)is type(b),'JSON type differs'
 if isinstance(a,dict):
  assert a.keys()==b.keys(),'JSON fields differ'
  for key in a:typed_equal(a[key],b[key])
 elif isinstance(a,list):
  assert len(a)==len(b),'JSON list length differs'
  for x,y in zip(a,b):typed_equal(x,y)
 else:assert a==b,'JSON value differs'
def validate(value):typed_equal(value,source)
validate(native)
rejected=0
for index,row in enumerate(native['states']):
 for key in ['annotation_hover','inks','selection','undo','redo']:
  candidate=copy.deepcopy(native);candidate['states'][index][key]='changed'
  try:validate(candidate)
  except AssertionError:rejected+=1
  else:raise AssertionError('accepted '+key)
for key,value in [('selection',True),('undo',0.0),('same_identity',1)]:
 candidate=copy.deepcopy(native);candidate['states'][0][key]=value
 try:validate(candidate)
 except AssertionError:rejected+=1
 else:raise AssertionError('accepted bool/int/float alias')
candidate=copy.deepcopy(native);candidate['states'][1]['annotation_hover'][1]=False
try:validate(candidate)
except AssertionError:rejected+=1
else:raise AssertionError('accepted boolean owner index')
for index in [0,6,13]:
 candidate=copy.deepcopy(native);inks=candidate['states'][index]['inks'];inks['label'],inks['dimension']=inks['dimension'],inks['label']
 try:validate(candidate)
 except AssertionError:rejected+=1
 else:raise AssertionError('accepted any-orange wrong-target ink')
weak=json.loads((p/'first40-native-weak.json').read_text())
try:validate(weak)
except AssertionError:pass
else:raise AssertionError('accepted retained non-target-specific weak candidate')
print(json.dumps(dict(exact_states=42,rejected_mutations=rejected,rejected_retained_weak_candidate=1,source_new_states=12,retained_source_states=30,scope='actual App/cache picks + target-specific CPU native plan; no GPU cache reuse or pixel claim')))
