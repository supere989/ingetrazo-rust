import copy,json,sys

def validate(n):
 for key in ('discard_finalizer_bytecode_refused','final_zero_with_cycle','preparation_refusal_session_recovers','unwind_restores_fields_and_retires_session'):
  assert type(n[key]) is bool and n[key] is True,key
 assert type(n['production_enabled']) is bool and n['production_enabled'] is False

n=json.load(open(sys.argv[1]));validate(n)
rejected=[]
for key in ('discard_finalizer_bytecode_refused','final_zero_with_cycle','production_enabled'):
 altered=copy.deepcopy(n);altered[key]=not altered[key]
 try:validate(altered)
 except AssertionError:rejected.append(key)
 else:raise AssertionError('accepted mutated '+key)
print(json.dumps({'finalizer_control':True,'negative_controls':rejected,'full_callback_safety':False,'production_enabled':False}))
