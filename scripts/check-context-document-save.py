#!/usr/bin/env python3
"""Compare full normalized actual save-handler traces and source reopens."""
import copy,json,sys
from pathlib import Path
p=Path(sys.argv[1]);original=json.loads((p/'original.json').read_text());native=json.loads((p/'native.json').read_text());reopened=json.loads((p/'reopened.json').read_text())
def validate(native,reopened,expected=original):
 for trace in [expected,native]:
  assert [row['case']for row in trace['rows']]==['save','save_as_cancel','save_as','failed'],'Missing, duplicate or reordered workflows'
  for row in trace['rows']:
   assert [phase['phase']for phase in row['phases']]==['after','undo','redo'],'Missing or duplicate history phase'
 assert native['rows']==expected['rows'],'Save handler state/history/camera/tool mismatch'
 assert native['invalid_refusal'] is True
 assert set(reopened)=={'save','save_as'}
 for row in expected['rows']:
  if row['case']in reopened:assert reopened[row['case']]==row['phases'][0]['state'],'Original reopen lost document geometry/identity/extension fields'
validate(native,reopened)
negatives=[]
for field in ['root_points','groups','camera_target','undo','depth','tool']:
 bad=copy.deepcopy(native);phase=bad['rows'][0]['phases'][0]
 if field in ['root_points','groups']:phase['state'][field]=[]
 elif field=='camera_target':phase[field]=[0,0,0]
 elif field=='tool':phase[field]='select'
 else:phase[field]+=1
 try:validate(bad,reopened)
 except AssertionError:negatives.append(field)
 else:raise AssertionError('undetected '+field)
bad=copy.deepcopy(reopened);bad['save']['groups'].pop()
try:validate(native,bad)
except AssertionError:negatives.append('reopened_group')
else:raise AssertionError('undetected reopened group')
for name in ['empty','truncated','duplicate','missing_phase']:
 bad=copy.deepcopy(native);expected=copy.deepcopy(original)
 for trace in [bad,expected]:
  if name=='empty':trace['rows']=[]
  elif name=='truncated':trace['rows'].pop()
  elif name=='duplicate':trace['rows'][1]=copy.deepcopy(trace['rows'][0])
  else:trace['rows'][0]['phases'].pop()
 try:validate(bad,reopened,expected)
 except AssertionError:negatives.append(name)
 else:raise AssertionError('undetected joint '+name)
print(json.dumps({'passed':True,'save_workflows':4,'state_history_camera_phases':12,'original_reopens':2,'invalid_refusal':True,'negative_controls':negatives}))
