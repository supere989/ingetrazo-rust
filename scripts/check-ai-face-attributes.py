import json,copy,sys
from pathlib import Path
p=Path(sys.argv[1])if len(sys.argv)>1 else Path(__file__).resolve().parents[1]/'docs/audits/ai-face-attributes'
o=json.loads((p/'original.json').read_text());n=json.loads((p/'native.json').read_text())
def validate(c):
 if len(c)!=12 or len({s['name']for s in c})!=12:raise AssertionError('inventory')
 if c!=o:raise AssertionError('attribute/history mismatch')
validate(n)
for i in range(6):
 c=copy.deepcopy(n)
 if i==0:c.pop()
 elif i==1:c[0]['faces'][0]['attrs']['vendor']['path']='changed'
 elif i==2:c[0]['faces'][0]['attrs']['color']=c[0]['faces'][0]['attrs']['color'][:3]
 elif i==3:c[2]['faces'][0]['attrs']['layer']='Default'
 elif i==4:c[1]['redo']=0
 else:c[5]['faces'][0]['attrs']['texture']['vendor']['material']='Named Paint'
 try:validate(c)
 except AssertionError:pass
 else:raise AssertionError('mutation accepted')
a=json.loads((p/'allocation.log').read_text().splitlines()[0]);assert a['pointer_stable']and a['actual_refusal_preserved_redo']and a['ledger_released'];assert a['history_max_allocation']<a['payload_bytes']
s=json.loads((p/'source-object-identity.json').read_text());ix={x['name']:x for x in s}
assert len(s)==8 and len(ix)==8
for name in ['append','redo','scope-alias-mutated']:
 assert ix[name]['held_is_live']and ix[name]['shared_alias']and ix[name]['shared_is_scope']
 assert ix[name]['tuple_type']=='tuple'and ix[name]['nonstring_keys']==['int','tuple']
assert ix['scope-mutated-while-undone']['held_shared_value']==['while-undone']
assert ix['redo']['live_shared_values']==[['while-undone'],['while-undone']]
assert ix['scope-alias-mutated']['live_shared_values']==[['while-undone','later'],['while-undone','later']]
assert ix['finish-group']['returned_is_placement']and ix['finish-redo']['returned_is_placement']
assert ix['finish-undo']['held_mesh_faces']==6
print('12 exact native attribute states; six mutations reject; actual allocator refusal/pointer controls PASS; eight ORIGINAL-ONLY object controls retained')
