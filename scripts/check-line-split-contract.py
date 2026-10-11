#!/usr/bin/env python3
"""Literal pinned split/weld flags and Qt long-skew authoring controls."""
import copy,json,sys
from pathlib import Path
ROSTER=['metadata-split-weld','long-skew-accepted','long-skew-refused']
def validate(s,n):
 assert s['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [c['case']for c in s['cases']]==ROSTER==[c['case']for c in n['cases']]
 for a,b in zip(s['cases'],n['cases'],strict=True):
  assert a['before']==b['before'] and a['after']==b['after'],(a['case'],a['after'],b['after'])
  assert a['redo']==a['after']
 assert s['cases'][1]['intersection']==s['cases'][1]['after']['vertices'][2]
 assert s['cases'][2]['intersection']is None
 assert s['cases'][0]['undo']==s['cases'][0]['before']
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text());validate(s,n)
rejected=[]
for kind in ['empty','duplicate','copied_hidden','copied_layer','stamped_weld','replaced_weld','lost_remote','raw_f64_point','missed_long_skew','admitted_far_skew']:
 m=copy.deepcopy(n)
 if kind=='empty':m['cases']=[]
 elif kind=='duplicate':m['cases'][1]=m['cases'][0]
 elif kind=='copied_hidden':m['cases'][0]['after']['edges'][2]['hidden']=True
 elif kind=='copied_layer':m['cases'][0]['after']['edges'][2]['layer']='Old'
 elif kind=='stamped_weld':m['cases'][0]['after']['edges'][0]['soft']=True
 elif kind=='replaced_weld':m['cases'][0]['after']['edges'][0]['old_identity']=None
 elif kind=='lost_remote':m['cases'][0]['after']['edges'][1]['hidden']=False
 elif kind=='raw_f64_point':m['cases'][1]['after']['vertices'][2][0]=.1234567
 elif kind=='missed_long_skew':m['cases'][1]['after']=m['cases'][1]['before']
 else:m['cases'][2]['after']=copy.deepcopy(m['cases'][1]['after'])
 try:validate(s,m)
 except (AssertionError,KeyError,IndexError):rejected.append(kind)
 else:raise AssertionError('negative accepted '+kind)
report={'source_pin':s['source_pin'],'cases':ROSTER,'exact_before_after_flags_and_old_object_identities':3,'negative_controls':rejected,'scope':'Actual pinned build_add_edge/History versus native core planner. Existing nine-case app History replay supplies separate consumer evidence; no OS gesture claim.'}
Path(sys.argv[3]).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
