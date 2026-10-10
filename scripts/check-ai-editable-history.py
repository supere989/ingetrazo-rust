import copy,json,sys
NAMES=['initial','edited','undone','redone','undone-again','redone-again']
def equal(a,b,path='root'):
 assert type(a)is type(b),(path,type(a).__name__,type(b).__name__)
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(path,'inventory')
  for k in a:equal(a[k],b[k],path+'.'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,'length')
  for i,(x,y)in enumerate(zip(a,b)):equal(x,y,path+'.'+str(i))
 else:assert a==b,(path,a,b)
def validate(s,n):
 assert [x['name']for x in s['states']]==NAMES
 assert [x['name']for x in n['states']]==NAMES
 equal(s['states'],n['states'])
 for k in ('redo_refusal_retains_history','execution_refusal_retains_redo','no_op_retains_redo','chronological_append_field_history','branch_truncation_retains_new_command','empty_history_no_admission'):
  assert type(n[k])is bool and n[k]is True,k
 for k in ('execution_refusal_retains_redo','no_op_retains_redo','chronological_append_field_history','branch_truncation_retains_new_command'):assert s[k]is True
 assert type(n['final_charge'])is int and n['final_charge']==0
 for k in ('full_mutation_history','production_enabled'):assert type(n[k])is bool and n[k]is False
s=json.load(open(sys.argv[1]));n=json.load(open(sys.argv[2]));validate(s,n)
mutations=[('position',lambda x:x['states'][2]['positions'][0].__setitem__(0,2.)),('face_order',lambda x:x['states'][2]['faces'].__setitem__(0,[2,1,0])),('position_identity',lambda x:x['states'][2].__setitem__('position_original',True)),('loop_identity',lambda x:x['states'][3].__setitem__('loop_original',True)),('attrs_identity',lambda x:x['states'][4].__setitem__('attrs_original',True)),('shared_alias',lambda x:x['states'][2].__setitem__('shared_identity',False)),('shared_value',lambda x:x['states'][2].__setitem__('shared_value',[])),('redo',lambda x:x['states'][2].__setitem__('redo',0)),('charge',lambda x:x.__setitem__('final_charge',1)),('quota',lambda x:x.__setitem__('redo_refusal_retains_history',False)),('branch',lambda x:x.__setitem__('branch_truncation_retains_new_command',False)),('empty_quota',lambda x:x.__setitem__('empty_history_no_admission',False)),('full_claim',lambda x:x.__setitem__('full_mutation_history',True)),('production',lambda x:x.__setitem__('production_enabled',True))]
rejected=[]
for name,mutate in mutations:
 x=copy.deepcopy(n);mutate(x)
 try:validate(s,x)
 except (AssertionError,KeyError):rejected.append(name)
 else:raise AssertionError('accepted '+name)
old=json.load(open(sys.argv[3]))
try:validate(s,old)
except (AssertionError,KeyError):old_rejected=True
else:raise AssertionError('accepted literal missing-history baseline')
print(json.dumps({'source_states_matched':6,'negative_controls':rejected,'literal_old_missing_history_rejected':old_rejected,'full_scripting_acceptance':False,'production_enabled':False}))
