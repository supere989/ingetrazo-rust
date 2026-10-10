import copy,json,sys

def validate(source,native):
 assert [s['name']for s in source['states']]==['initial','edited','undone','redone','undone-again','redone-again']
 assert [s['name']for s in native['states']]==['initial','edited','refused']
 for s,n in zip(source['states'][:2],native['states'][:2]):
  assert s['positions']==n['mesh']['vertices']
  assert s['faces']==[f['vertices']for f in n['mesh']['faces']]
 assert native['states'][2]['mesh']==native['states'][1]['mesh']
 for key in ('raw_weld_key_identity','new_key_distinct_coincident_vertex','invalid_loop_refusal_restores_vm_and_native','unchanged_membership_only','identity_order_and_mixed_append_refused'):
  assert type(native[key])is bool and native[key]is True,key
 assert type(native['final_charge'])is int and native['final_charge']==0
 for key in ('controller_history','production_enabled'):assert type(native[key])is bool and native[key]is False
 assert source['old_key_returns_moved_identity']is True
 assert source['new_key_creates_distinct_coincident_vertex']is True
 assert source['source_registry_rebuilt_on_vector_mutation']is False
s=json.load(open(sys.argv[1]));n=json.load(open(sys.argv[2]));validate(s,n)
rejected=[]
mutations=[('position',lambda x:x['states'][1]['mesh']['vertices'][0].__setitem__(0,3)),('loop',lambda x:x['states'][1]['mesh']['faces'][0].__setitem__('vertices',[0,1,2])),('rollback',lambda x:x['states'][2]['mesh']['vertices'][0].__setitem__(0,4)),('registry',lambda x:x.__setitem__('raw_weld_key_identity',False)),('charge',lambda x:x.__setitem__('final_charge',1)),('history_claim',lambda x:x.__setitem__('controller_history',True)),('production',lambda x:x.__setitem__('production_enabled',True)),('inventory',lambda x:x['states'].pop()),('identity_guard',lambda x:x.__setitem__('identity_order_and_mixed_append_refused',False))]
for name,change in mutations:
 x=copy.deepcopy(n);change(x)
 try:validate(s,x)
 except (AssertionError,KeyError):rejected.append(name)
 else:raise AssertionError('accepted '+name)
print(json.dumps({'projected_source_states':2,'source_history_states_retained_not_native_accepted':6,'negative_controls':rejected,'native_history_accepted':False,'production_enabled':False}))
