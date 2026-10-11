import json,sys,copy

def exact(a,b):
 assert type(a) is type(b), (type(a),type(b))
 if isinstance(a,dict):
  assert a.keys()==b.keys()
  for k in a:exact(a[k],b[k])
 elif isinstance(a,list):
  assert len(a)==len(b)
  for x,y in zip(a,b):exact(x,y)
 else:assert a==b

def validate(a,b):
 assert set(a)==set(b)=={'states'}
 assert len(a['states'])==len(b['states'])==12
 exact(a,b)

a=json.load(open(sys.argv[1]));b=json.load(open(sys.argv[2]));validate(a,b)
for kind in ['winding','hole','shade','soft','history','rollback','detached','type']:
 c=copy.deepcopy(b)
 if kind=='winding':c['states'][0]['groups'][0]['faces'][0]['vertices'].reverse()
 elif kind=='hole':c['states'][0]['groups'][0]['faces'][0]['holes']=[[0,1,2]]
 elif kind=='shade':c['states'][0]['groups'][0]['faces'][0]['attrs']['color'][0]+=0.01
 elif kind=='soft':c['states'][0]['groups'][0]['edges'][0]['soft']=not c['states'][0]['groups'][0]['edges'][0]['soft']
 elif kind=='history':c['states'][2]['undo']+=1
 elif kind=='rollback':c['states'][1]['groups'].append(c['states'][0]['groups'][0])
 elif kind=='type':c['states'][0]['groups'][0]['edges'][0]['soft']=int(c['states'][0]['groups'][0]['edges'][0]['soft'])
 else:c['states'][3]['groups'][0]['faces'][0]['attrs'].pop('vendor')
 try:validate(a,c)
 except AssertionError:pass
 else:raise AssertionError('accepted '+kind)
print('12 full house/app states; 8 rejecting controls PASS')
