"""Strict complete immediate document/history observations, executable negatives."""
import copy,json,sys

def validate(original,native):
 assert set(original)=={'states'}
 assert set(native)=={'states','final_charge'} and native['final_charge']==0
 assert len(original['states'])==len(native['states'])==10
 assert native['states']==original['states']

a=json.load(open(sys.argv[1]));b=json.load(open(sys.argv[2]));validate(a,b)
for kind in ['membership','pose','attrs','history','charge']:
 c=copy.deepcopy(b)
 if kind=='membership':c['states'][5]['groups']=[]
 elif kind=='pose':c['states'][5]['groups'][1]['positions'][0][2]+=1
 elif kind=='attrs':c['states'][5]['groups'][1]['attrs'][0]['mat']='Lost'
 elif kind=='history':c['states'][2]['undo']+=1
 else:c['final_charge']=1
 try:validate(a,c)
 except AssertionError:pass
 else:raise AssertionError('accepted '+kind)
print('10 immediate states and 5 negative controls PASS')
