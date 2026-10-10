import copy,json,sys
NAMES=['initial','mixed','undone','redone','undone-again','redone-again']
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
 assert s['appended_reference_identity'] is True
 assert n['appended_reference_identity'] is True and type(n['appended_reference_identity']) is bool
 assert s['chronological_undo_redo'] is True
 assert s['execution_refusal_preserves_redo'] is True
 for k in ('quota_preserves_command','chronological_undo_redo','execution_refusal_preserves_redo'):assert type(n[k])is bool and n[k]is True
 assert type(n['final_charge'])is int and n['final_charge']==0
 assert type(n['production_enabled'])is bool and n['production_enabled']is False
s=json.load(open(sys.argv[1]));n=json.load(open(sys.argv[2]));validate(s,n)
mutations=[('appended_identity',lambda x:x.__setitem__('appended_reference_identity',False)),('position',lambda x:x['states'][2]['positions'][0].__setitem__(0,2.)),('append',lambda x:x['states'][1]['faces'].pop()),('loop',lambda x:x['states'][1]['faces'].__setitem__(0,[0,1,2])),('held_vertex',lambda x:x['states'][2].__setitem__('vertex_live',False)),('wrapper',lambda x:x['states'][3].__setitem__('loop_original',True)),('shared',lambda x:x['states'][2].__setitem__('shared_value',[])),('undo',lambda x:x['states'][2].__setitem__('undo',2)),('redo',lambda x:x['states'][2].__setitem__('redo',0)),('quota',lambda x:x.__setitem__('quota_preserves_command',False)),('chronological',lambda x:x.__setitem__('chronological_undo_redo',False)),('charge',lambda x:x.__setitem__('final_charge',1)),('execution_error',lambda x:x.__setitem__('execution_refusal_preserves_redo',False)),('production',lambda x:x.__setitem__('production_enabled',True))]
rejected=[]
for name,mutate in mutations:
 x=copy.deepcopy(n);mutate(x)
 try:validate(s,x)
 except (AssertionError,KeyError):rejected.append(name)
 else:raise AssertionError('accepted '+name)
print(json.dumps({'source_states_matched':6,'negative_controls':rejected,'production_enabled':False,'full_scripting_acceptance':False}))
