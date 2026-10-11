import copy,json,sys
from pathlib import Path
NAMES=['cube-cap','triangle-long-wall','triangle-axis-wall','triangle-end','wedge-roof','wedge-xwall','wedge-ywall','gable-roof','step-mouth','unattached-sheet','partial-parallel','disjoint-parallel','ctrl-cap']
GAPS={'unattached-sheet','disjoint-parallel'}
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text());v=json.loads(Path(sys.argv[3]).read_text());baseline=json.loads(Path(sys.argv[4]).read_text())
def ring(p):return min(tuple(tuple(x)for x in p[i:]+p[:i])for i in range(len(p)))
def fields(fs):return sorted((ring(f['vertices']),tuple(sorted(ring(h)for h in f['holes'])),json.dumps(f['attrs'],sort_keys=True),f['interior'])for f in fs)
def validate(s,n):
 assert s['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [c['name']for c in s['cases']]==NAMES==[c['name']for c in n['cases']]
 assert n['nested_history_exact']is True
 assert n['cases'][-1]['primitive_error'].startswith('Ctrl copy would leave an invalid solid:') and n['cases'][-1]['primitive_atomic_on_error']is True
 for a,b in zip(s['cases'],n['cases'],strict=True):
  assert a['limit']==b['limit'] and a['distance']==b['distance'],a['name']+' limit/preview mismatch'
  assert b['error']is None and b['preview_cancel_exact']is True and b['app_history_exact']is True
  assert b['status']==(a['status']or''),a['name']+' status mismatch'
  if a['name']in GAPS:
   assert fields(a['result'])!=fields(b['result'])
   assert b['result']==baseline[a['name']],a['name']+' remaining mismatch changed'
  else:assert fields(a['result'])==fields(b['result']),a['name']+' topology/attrs/interior mismatch'
 assert [c['name']for c in s['reused_limit_controls']]==['wider-punchable-sheet','annular-support']==[c['name']for c in n['reused_limit_controls']]
 for a,b in zip(s['reused_limit_controls'],n['reused_limit_controls'],strict=True):assert a['limit']==b['limit'] and a['clamped_distance']==b['distance'] and b['preview_model_exact']is True
 assert [c['value']for c in v['cases']]==[99.,-99.]==[c['value']for c in n['vcb']]
 for a,b in zip(v['cases'],n['vcb'],strict=True):
  assert a['accepted']is True and a['undo']==1 and b['history_exact']is True
  assert fields(a['result'])==fields(b['result']) and (a['status']or'')==b['status']
validate(s,n)
rejected=[]
for kind in ['missing_inventory','wrong_lateral_limit','missing_zero_exit','unclamped_ctrl','lost_face','reversed_cap','wrong_interior','lost_history','wrong_status','wrong_attributes','missing_vcb_direction']:
 bad=copy.deepcopy(n)
 if kind=='missing_inventory':bad['cases'].pop()
 elif kind=='wrong_lateral_limit':bad['cases'][7]['limit']+=.001
 elif kind=='missing_zero_exit':bad['cases'][1]['limit']=None
 elif kind=='unclamped_ctrl':bad['cases'][-1]['distance']=-99
 elif kind=='lost_face':bad['cases'][7]['result'].pop()
 elif kind=='reversed_cap':bad['cases'][0]['result'][0]['vertices'].reverse()
 elif kind=='wrong_interior':bad['cases'][-1]['result'][2]['interior']=False
 elif kind=='lost_history':bad['nested_history_exact']=False
 elif kind=='wrong_status':bad['cases'][0]['status']=''
 elif kind=='wrong_attributes':bad['cases'][7]['result'][0]['attrs']={'vendor':900}
 else:bad['vcb'][0]['result']=bad['vcb'][1]['result']
 try:validate(s,bad)
 except AssertionError:rejected.append(kind)
 else:raise AssertionError('accepted corruption '+kind)
rows=[{'name':a['name'],'limit_exact':True,'preview_distance_exact':True,'source_faces':len(a['result']),'native_faces':len(b['result']),'full_oriented_fields_exact':a['name']not in GAPS,'history_cancel_exact':True}for a,b in zip(s['cases'],n['cases'])]
report={'bounded_gate_passed':True,'full_pushpull_parity':False,'cases':rows,'negative_controls':rejected,'signed_vcb_cases':2,'nested_history_exact':True,'remaining_gaps':{'unattached-sheet':'Five free inward cap/wall rings have opposite winding; no normalization.','disjoint-parallel':'Original global flat healing deletes every face; native preserves flat cap and disconnected sheet.'},'scope':'13 exact source inward-limit/preview/status decisions, 11 exact final oriented face/holes/attrs/interior inventories, 2 retained mismatches; actual app handlers/VCB/Cancel/Undo/Redo/nested Finish. No OS gesture or full algorithm corpus claim.'}
Path(sys.argv[5]).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
