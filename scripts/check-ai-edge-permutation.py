"""Strict combined identity-order and registry-fed append acceptance."""
import copy,json,sys
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
def validate(e,a):
 names=[f'{case}/{state}' for case in ['edge-only','combined-orders','combined-pose-loop'] for state in ['edited','undone','redone','repeat-undone','repeat-redone','append-shared','append-undone','chronological-order-undone','chronological-order-redone','append-redone']]
 assert [s['name']for s in e['states']]==names
 exact(e,a)
def failure(a):
 exact({'actual_native_allocation_refusals':12,'graph_history_and_charges_preserved':True,'bigint_and_vm_heap_failure_included':False,'production_enabled':False},a)
e,a,f=map(read,sys.argv[1:4]);validate(e,a);failure(f)
rejected=[]
mutations=[('missing-state',lambda x:x['states'].pop()),('edge-order',lambda x:x['states'][0]['native_endpoints'].reverse()),('incidence-remap',lambda x:x['states'][0]['native_incidence'][0].pop()),('radial-remap',lambda x:x['states'][10]['native_radial'][0].append(99)),('registry-fed-append',lambda x:x['states'][15]['native_endpoints'][-1].__setitem__(0,99)),('pose',lambda x:x['states'][20]['native_positions'][0].__setitem__(2,99.0)),('loop',lambda x:x['states'][20]['native_faces'][0]['loop'].reverse()),('redo',lambda x:x['states'][1].__setitem__('redo',0)),('bool-id',lambda x:x['states'][0]['native_incidence'][0].__setitem__(0,False))]
for name,mutate in mutations:
 b=copy.deepcopy(a);mutate(b)
 try:validate(e,b)
 except AssertionError:rejected.append(name)
 else:raise AssertionError('Mutation accepted: '+name)
for name,mutate in [('no-refusal',lambda x:x.__setitem__('actual_native_allocation_refusals',0)),('lost-rollback',lambda x:x.__setitem__('graph_history_and_charges_preserved',False))]:
 b=copy.deepcopy(f);mutate(b)
 try:failure(b)
 except AssertionError:rejected.append(name)
 else:raise AssertionError('Mutation accepted: '+name)
print(json.dumps({'exact_full_states':30,'actual_native_allocation_refusals':12,'negative_controls_rejected':rejected,'production_enabled':False}))
