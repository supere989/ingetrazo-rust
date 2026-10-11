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
 assert len(a['states'])==len(b['states'])==19
 exact(a,b)

a=json.load(open(sys.argv[1]));b=json.load(open(sys.argv[2]));validate(a,b)
for kind in ['alias','packs','replacement','detached','undo','component','placement','type','restore','paint','inventory']:
 c=copy.deepcopy(b)
 if kind=='alias':c['states'][0]['aliases'][0][1]=False
 elif kind=='packs':c['states'][0]['packed_meshes']+=1
 elif kind=='replacement':c['states'][5]['groups'][0]['positions']=c['states'][5]['groups'][1]['positions']
 elif kind=='detached':c['states'][3]['groups'][0]['faces'][0]['attrs']['color'][0]+=0.1
 elif kind=='undo':c['states'][2]['undo']+=1
 elif kind=='component':c['states'][0]['groups'][0]['component']=False
 elif kind=='placement':c['states'][0]['groups'][0]['placed']=False
 elif kind=='type':c['states'][0]['aliases'][0][1]=1
 elif kind=='restore':c['states'][18]['groups'][0]['faces'][0]['vertices'].reverse()
 elif kind=='paint':c['states'][18]['groups'][0]['faces'][0]['attrs']['opacity']=0.5
 else:c['states'][0]['groups'][0].pop('edges')
 try:validate(a,c)
 except AssertionError:pass
 else:raise AssertionError('accepted '+kind)
print('19 full typed imported sharing/app states +11 rejected controls PASS')
