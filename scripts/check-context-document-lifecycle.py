#!/usr/bin/env python3
"""Strict, fixed-roster lifecycle/pending-gesture comparison; no fitting/tolerances."""
import json,copy,sys
from pathlib import Path
p=Path(sys.argv[1])
source=json.loads((p/'original.json').read_text());native=json.loads((p/'native.json').read_text())
sp=json.loads((p/'original-pending.json').read_text());np=json.loads((p/'native-pending.json').read_text())
expected=['new_cancel','open_cancel','open_picker_cancel','open_discard','open_failed','new_discard','close_cancel','close_discard','close_save','close_save_cancel','close_save_failed']
pending=['line-first','line-chain','rectangle-first']
def equal(a,b,path=''):
 if type(a)!=type(b) and not(isinstance(a,(float,int)) and isinstance(b,(float,int))):raise AssertionError(path+': type mismatch')
 if isinstance(a,dict):
  assert a.keys()==b.keys(),path+': keys mismatch'
  for k in a:equal(a[k],b[k],path+'/'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),path+': length mismatch'
  for i,(x,y)in enumerate(zip(a,b)):equal(x,y,path+'/'+str(i))
 else:assert a==b,path+': '+repr(a)+' != '+repr(b)
def validate(s,n,ps,pn):
 for a in [s,n]:
  assert [r['case']for r in a['rows']]==expected,'lifecycle roster'
  for r in a['rows']:assert [v['phase']for v in r['phases']]==(['after']if r['case']in ['close_discard','close_save']else ['after','undo','redo']),'lifecycle phase roster'
 for a in [ps,pn]:
  assert [r['case']for r in a['rows']]==pending,'pending roster'
  for r in a['rows']:assert [v['phase']for v in r['phases']]==['after','continued','undo','redo'],'pending phase roster'
 equal(s,n,'lifecycle');equal(ps,pn,'pending')
# Compare every field and keep strict failures as failures; fixed rosters prevent
# coincidentally matching empty or truncated traces from passing.
def differences(a,b,path=''):
 if isinstance(a,dict)and isinstance(b,dict):
  for k in sorted(a.keys()|b.keys()):
   if k not in a or k not in b:yield path+'/'+k
   else:yield from differences(a[k],b[k],path+'/'+k)
 elif isinstance(a,list)and isinstance(b,list):
  if len(a)!=len(b):yield path+'/length'
  for i,(x,y)in enumerate(zip(a,b)):yield from differences(x,y,path+'/'+str(i))
 elif a!=b:yield path
# A copy of the original must satisfy the fixed roster and exact comparison.
validate(source,copy.deepcopy(source),sp,copy.deepcopy(sp))
validate(native,copy.deepcopy(native),np,copy.deepcopy(np))
base=set(differences(source,native,'lifecycle'))|set(differences(sp,np,'pending'))
reopens=json.loads((p/'reopened.json').read_text())
assert set(reopens)=={'close_save',*pending},'reopen roster'
assert reopens['close_save']==source['rows'][8]['phases'][0]['state']
for row in sp['rows']:assert reopens[row['case']]==row['phases'][0]['state'],row['case']+' reopen'
neg=[]
for name in ['prompt_depth','scene','history','camera','anchor','continued','empty','missing','duplicate','missing_phase']:
 n=copy.deepcopy(native);pn=copy.deepcopy(np);s=copy.deepcopy(source);ps=copy.deepcopy(sp)
 if name=='prompt_depth':n['rows'][0]['prompt_depths']=[2]
 elif name=='scene':n['rows'][0]['phases'][0]['state']['groups']=[]
 elif name=='history':n['rows'][0]['phases'][0]['undo']+=1
 elif name=='camera':n['rows'][0]['phases'][0]['camera_target']=[0,0,0]
 elif name=='anchor':pn['rows'][0]['anchor_after']=None
 elif name=='continued':pn['rows'][0]['phases'][1]['state']['root_mesh']['edges']=[]
 elif name=='empty':s['rows']=[];n['rows']=[]
 elif name=='missing':s['rows'].pop();n['rows'].pop()
 elif name=='duplicate':s['rows'][1]=s['rows'][0];n['rows'][1]=n['rows'][0]
 else:s['rows'][0]['phases'].pop();n['rows'][0]['phases'].pop()
 if name in ['empty','missing','duplicate','missing_phase']:
  try:validate(s,n,ps,pn)
  except AssertionError:neg.append(name)
  else:raise AssertionError('undetected '+name)
 else:
  changed=set(differences(s,n,'lifecycle'))|set(differences(ps,pn,'pending'))
  assert changed-base,'undetected new mismatch '+name
  neg.append(name)
report={'passed':not base,'lifecycle_workflows':len(expected),'lifecycle_passed':source==native,'pending_workflows':len(pending),'pending_full_parity':sp==np,'state_history_phases':sum(len(r['phases'])for r in source['rows']+sp['rows']),'original_reopens':len(reopens),'negative_controls':neg,'strict_failures':sorted(base)}
print(json.dumps(report));sys.exit(0 if report['passed']else 1)
