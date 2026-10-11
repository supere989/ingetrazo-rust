"""Exact per-boundary radial graph inventory, including literal old dedup failure."""
import json,sys,copy
from pathlib import Path
def read(path):return json.loads(Path(path).read_text())
def exact(a,b,path='$'):
 assert type(a)is type(b),(path,'type')
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(path,'keys')
  for k in a:exact(a[k],b[k],path+'.'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,'length')
  for i,(x,y)in enumerate(zip(a,b)):exact(x,y,path+f'[{i}]')
 else:assert a==b,(path,a,b)
def validate(expected,actual):
 assert len(expected['states'])==21
 assert len({s['name']for s in expected['states']})==21
 exact(expected,actual)
e,a,old=map(read,sys.argv[1:4]);validate(e,a)
rejected=[]
for name,changed in [('literal-old-dedup',old),('lost-state',dict(a,states=a['states'][:-1]))]:
 try:validate(e,changed)
 except AssertionError:rejected.append(name)
 else:raise AssertionError('Mutation accepted: '+name)
for name,change in [
 ('radial-multiplicity',lambda x:x['states'][6]['radial'][0].pop()),
 ('native-multiplicity',lambda x:x['states'][6]['native_radial'][0].pop()),
 ('self-edge-endpoint',lambda x:x['states'][3]['endpoints'][0].__setitem__(1,None)),
 ('repeated-loop',lambda x:x['states'][9]['faces'][0]['loop'].pop()),
 ('lost-redo',lambda x:x['states'][1].__setitem__('redo',0)),
 ('bool-incidence',lambda x:x['states'][3]['incidence'][0].__setitem__(0,False))]:
 changed=copy.deepcopy(a);change(changed)
 try:validate(e,changed)
 except AssertionError:rejected.append(name)
 else:raise AssertionError('Mutation accepted: '+name)
print(json.dumps({'exact_full_states':21,'negative_controls_rejected':rejected,'production_enabled':False}))
