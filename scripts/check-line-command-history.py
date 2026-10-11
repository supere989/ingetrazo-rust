#!/usr/bin/env python3
"""Exact command classification, raw forward/Undo/Redo order and lifetime."""
import json,copy,sys
from pathlib import Path
ROSTER=['bare-empty','bare-existing','bare-duplicate','bare-off-plane-face','snapshot-plane-face','snapshot-curve','snapshot-crossing','snapshot-closing','midchain-branch']
def validate(source,native):
 assert [r['case']for r in source]==ROSTER
 assert [r['case']for r in native]==ROSTER
 for s,n in zip(source,native):
  assert set(s)==set(n)
  for field in ['a','b','class','before','undo','anchor','history']:assert s[field]==n[field],(s['case'],field,s[field],n[field])
  assert n['redo']==n['after'],(n['case'],'native Redo')
  assert n['cancel']==(n['branch']if n['branch']is not None else n['redo'])
  assert s==n,(s['case'],'complete raw history')
def reject(s,n):
 try:validate(s,n)
 except (AssertionError,TypeError,KeyError):return
 raise AssertionError('negative accepted')
p=Path(sys.argv[1]);s=json.loads((p/'original.json').read_text());n=json.loads((p/'native.json').read_text());validate(s,n)
count=0
for i,field,value in [(0,'class','SnapshotCompound'),(0,'undo',s[0]['before']),(4,'class','AddEdgeCommand'),(4,'undo',s[4]['after']),(5,'undo',s[5]['after']),(6,'undo',s[6]['after']),(8,'branch',None),(2,'history',[0,0])]:
 m=copy.deepcopy(n);m[i][field]=value;reject(s,m);count+=1
for index,field in [(4,'after'),(6,'after'),(7,'after')]:
 m=copy.deepcopy(n);state=m[index][field]
 if index==4:state['edges']=state['edges'][1:]+state['edges'][:1]
 elif index==6:
  state['vertices'][2],state['vertices'][3]=state['vertices'][3],state['vertices'][2]
  state['edges']=[[3 if v==2 else 2 if v==3 else v for v in edge]for edge in state['edges']]
 else:state['faces'][0]=state['faces'][0][1:]+state['faces'][0][:1]
 m[index]['redo']=copy.deepcopy(state);m[index]['cancel']=copy.deepcopy(state)
 reject(s,m);count+=1
for bad in [n[:-1],list(reversed(n)),n+[n[-1]]]:reject(s,bad);count+=1
print(json.dumps({'cases':len(s),'exact_bare_history_cases':sum(r['class']=='AddEdgeCommand'for r in s),'exact_command_and_undo_cases':len(s),'exact_forward_raw_cases':len(s),'negative_controls':count,'compound_forward_raw_differences':[a['case']for a,b in zip(s,n)if a['class']=='SnapshotCompound'and a['after']!=b['after']]}))
