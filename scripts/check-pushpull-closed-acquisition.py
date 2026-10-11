# SPDX-License-Identifier: GPL-3.0-or-later
import copy,json,sys
from pathlib import Path
NAMES=['reversed-cube-cap','reversed-gable-roof','reversed-cube-two-sided','annulus-flush-neighbour','partition-cap-inward']
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text())
def ring(r):return min(tuple(tuple(p)for p in r[i:]+r[:i])for i in range(len(r)))
def fields(fs):return sorted((ring(f['vertices']),tuple(sorted(ring(h)for h in f['holes'])),json.dumps(f['attrs'],sort_keys=True),f['interior'])for f in fs)
def check(value):
 assert s['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [c['name']for c in s['cases']]==NAMES==[c['name']for c in value['cases']]
 for index,(a,b)in enumerate(zip(s['cases'],value['cases'],strict=True)):
  assert b['normal']==a['normal'] and b['error']is None
  assert b['acquisition_undo']==0 and b['commit_undo']==1
  for stage in ['acquired','cancel']:assert fields(a[stage])==fields(b[stage])
  if index<3:
   assert b['acquisition_invalidated']is True
   for stage in ['result','undo','redo']:assert fields(a[stage])==fields(b[stage])
check(n);negative=[]
for kind in ['empty','duplicate','wrong_normal','wrong_acquisition','cancel_reverts','lost_back_paint','missing_wall','inward_commit','wrong_undo','lost_redo','extra_history','lost_invalidation']:
 v=copy.deepcopy(n);c=v['cases'][2]
 if kind=='empty':v['cases']=[]
 elif kind=='duplicate':v['cases'][0]=v['cases'][1]
 elif kind=='wrong_normal':c['normal']=[0,0,-1]
 elif kind=='wrong_acquisition':c['acquired'][0]['vertices'].reverse()
 elif kind=='cancel_reverts':c['cancel']=copy.deepcopy(s['cases'][2]['source'])
 elif kind=='lost_back_paint':next(f for f in c['result']if f['attrs'].get('back'))['attrs'].pop('back')
 elif kind=='missing_wall':c['result'].pop()
 elif kind=='inward_commit':c['result'][0]['vertices'][0][2]+=.5
 elif kind=='wrong_undo':c['undo']=copy.deepcopy(s['cases'][2]['source'])
 elif kind=='lost_redo':c['redo']=c['undo']
 elif kind=='extra_history':c['acquisition_undo']=1
 else:c['acquisition_invalidated']=False
 try:check(v)
 except AssertionError:negative.append(kind)
 else:raise AssertionError('accepted '+kind)
remaining=[]
for a,b in zip(s['cases'][3:],n['cases'][3:],strict=True):
 remaining.append(dict(name=a['name'],source_faces=len(a['result']),native_faces=len(b['result']),exact_fields=fields(a['result'])==fields(b['result']),source_distance=a['distance'],source_top_out=a['top_out'],source_status=a['status']))
report=dict(acquisition_cases=5,exact_closed_acquisition_model_cases=3,exact_app_cancel_undo_redo_cases=3,negative_controls=negative,remaining=remaining,non_ctrl_accepted_top_out_proved=False,scope='Actual source entry/commit versus native app dispatch/VCB; oriented face/holes/attributes/interior inventories, no renderer or OS gesture claim')
Path(sys.argv[3]).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
