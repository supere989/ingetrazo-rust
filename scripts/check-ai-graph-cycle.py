#!/usr/bin/env python3
import copy,json,pathlib,sys
expected_names=['faces','layer','curve','soft','hidden']
def validate(original,native):
 expected={'generated_face_vertex_edge_cycle_collected':True,'opaque_slot_cycles_collected':expected_names,'external_alias_retains_cycle':True,'actual_cycle_survives_without_roots_before_gc':True,'actual_payloads_destroyed_after_gc':True,'controller_drop_preserves_other_scope':True,'actual_final_live_payloads':[0,0],'production_enabled':False,'total_heap_budget_claimed':False}
 def exact(a,b):
  if type(a)is not type(b):return False
  if isinstance(a,dict):return a.keys()==b.keys() and all(exact(a[k],b[k])for k in a)
  if isinstance(a,list):return len(a)==len(b) and all(exact(x,y)for x,y in zip(a,b))
  return a==b
 if not exact(original,expected):raise ValueError('Incomplete original graph lifetime inventory')
 if not exact(native,original):raise ValueError('Native graph lifetime inventory mismatch')
original=json.loads(pathlib.Path(sys.argv[1]).read_text());native=json.loads(pathlib.Path(sys.argv[2]).read_text());validate(original,native)
negatives=[]
for key,value in [('opaque_slot_cycles_collected',expected_names[1:]),('external_alias_retains_cycle',False),('actual_cycle_survives_without_roots_before_gc',False),('actual_payloads_destroyed_after_gc',False),('controller_drop_preserves_other_scope',False),('actual_final_live_payloads',[2,1]),('total_heap_budget_claimed',True)]:
 candidate=copy.deepcopy(native);candidate[key]=value
 try:validate(original,candidate)
 except ValueError:negatives.append(key)
 else:raise AssertionError('Accepted invalid '+key)
print(json.dumps({'exact_original_native':True,'rejected_mutations':negatives}))
