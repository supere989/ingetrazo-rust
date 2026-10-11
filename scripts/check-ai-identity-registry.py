"""Owner-only real-reference authority: exact source facts and native refusal/lifetime."""
import json,copy,sys
from pathlib import Path
def read(p):return json.loads(Path(p).read_text())
def exact(a,b,p='$'):
 assert type(a)is type(b),(p,'type')
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(p,'keys')
  for k in a:exact(a[k],b[k],p+'.'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(p,'length')
  for i,(x,y)in enumerate(zip(a,b)):exact(x,y,p+f'[{i}]')
 else:assert a==b,(p,a,b)
expected={'genuine_vertex_identity':True,'detached_controller_lifetime':True,'duplicate_key_last_wins':True,'raw_move_preserves_old_key':True,'stale_and_foreign_refused':True,'foreign_vm_refused':True,'external_handles_keep_payloads_alive':[3,3],'prepare_discard_charge_released':True,'quota_refusal_preserves_registry':True,'rejected_candidate_finalizes_only_after_unlocked_drop':True,'native_finalizer_calls':1,'native_finalizer_under_owner_borrow':0,'actual_map_allocation_refusals':1,'actual_graph_payloads_final':[0,0],'final_ledger':0,'source_weld_lookup_complete':False,'native_replacement_complete':False,'production_enabled':False}
def validate(source,native):
 exact({'observations':{k:True for k in ['genuine_vertex_identity','raw_move_preserves_old_key','detached_controller_lifetime','duplicate_key_last_wins']},'production_enabled':False},source)
 exact(expected,native)
 exact(source['observations'],{k:native[k]for k in source['observations']})
s,n=map(read,sys.argv[1:3]);validate(s,n);rejected=[]
for key,value in [('genuine_vertex_identity',False),('raw_move_preserves_old_key',False),('foreign_vm_refused',False),('native_finalizer_under_owner_borrow',1),('native_finalizer_calls',0),('actual_graph_payloads_final',[3,3]),('external_handles_keep_payloads_alive',[0,0]),('actual_map_allocation_refusals',0),('final_ledger',False),('production_enabled',True)]:
 altered=copy.deepcopy(n);altered[key]=value
 try:validate(s,altered)
 except AssertionError:rejected.append(key)
 else:raise AssertionError('Accepted mutation: '+key)
malformed=copy.deepcopy(s);malformed['observations'].pop('raw_move_preserves_old_key')
try:validate(malformed,n)
except AssertionError:rejected.append('source-inventory')
else:raise AssertionError('Accepted malformed source')
print(json.dumps({'exact_source_identity_observations':4,'rejected_candidate_finalizes_only_after_unlocked_drop':True,'native_finalizer_calls':1,'native_finalizer_under_owner_borrow':0,'actual_map_allocation_refusals':1,'negative_controls_rejected':rejected,'production_enabled':False}))
