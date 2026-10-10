import json,copy,sys
from pathlib import Path
p=Path(sys.argv[1])if len(sys.argv)>1 else Path(__file__).resolve().parents[1]/'docs/audits/ai-vm-face-identity'
s=json.loads((p/'original-full-object-contract.json').read_text());n=json.loads((p/'native.json').read_text())
# Precisely declared Face phase; remaining Group source controls are retained.
assert len(s)==8 and [x['name']for x in s[5:]]==['finish-group','finish-undo','finish-redo']
def validate(c):
 if c['face_states']!=s[:5]:raise AssertionError('VM object/value/lifetime mismatch')
 for k in ['caught_handle_admission_discards','redo_preserved_on_refusal','detached_ordinal_reuse_identity','owner_ledger_released']:
  if c.get(k)is not True:raise AssertionError('host controller invariant')
 for k in ['native_attribute_projection','full_mesh_api','preexisting_mutation_rollback','compiler_native_wall_time_bound','production_enabled']:
  if c.get(k)is not False:raise AssertionError('scope boundary changed')
validate(n)
for i in range(8):
 c=copy.deepcopy(n)
 if i==0:c['face_states'].pop()
 elif i==1:c['face_states'][3]['held_is_live']=False
 elif i==2:c['face_states'][3]['shared_alias']=False
 elif i==3:c['face_states'][3]['tuple_type']='list'
 elif i==4:c['face_states'][3]['nonstring_keys']=['str','str']
 elif i==5:c['face_states'][3]['live_shared_values']=[[],[]]
 elif i==6:c['caught_handle_admission_discards']=False
 else:c['detached_ordinal_reuse_identity']=False
 try:validate(c)
 except AssertionError:pass
 else:raise AssertionError('negative accepted')
print('five genuine VM Face states exact; caught admission/detached ordinal/ledger controls PASS; eight negatives reject; three Group source states required next')
