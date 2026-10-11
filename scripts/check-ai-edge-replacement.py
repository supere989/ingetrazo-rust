"""Strict shared validator with executed mutation rejection controls."""
import copy,json,sys

def exact(a,b,path='root'):
 if type(a)is not type(b):raise AssertionError((path,type(a).__name__,type(b).__name__))
 if isinstance(a,dict):
  if a.keys()!=b.keys():raise AssertionError((path,'inventory'))
  for k in a:exact(a[k],b[k],path+'.'+k)
 elif isinstance(a,list):
  if len(a)!=len(b):raise AssertionError((path,'extent'))
  for i,(x,y)in enumerate(zip(a,b)):exact(x,y,path+f'[{i}]')
 elif a!=b:raise AssertionError((path,a,b))
def validate(original,native):
 exact(original['states'],native['states'])
 exact(original['production_enabled'],native['production_enabled'])
 for key,value in {'quota_refusal_preserves_graph_history_and_charge':True,'detached_endpoint_refusal_preserves_graph_history_and_charge':True,'final_charge':0}.items():exact(value,native[key])
 exact(set(native),{'states','production_enabled','quota_refusal_preserves_graph_history_and_charge','detached_endpoint_refusal_preserves_graph_history_and_charge','final_charge'})
 if len(native['states'])!=18:raise AssertionError('required six cases/three phases')
def main():
 original=json.load(open(sys.argv[1]));native=json.load(open(sys.argv[2]));validate(original,native);rejected=[]
 for key in ['endpoints','incidence','radial','native_incidence','native_metadata','held_edge_live','held_endpoint_ids','held_edge_incidence','undo']:
  altered=copy.deepcopy(native);altered['states'][0][key]=None
  try:validate(original,altered)
  except (AssertionError,KeyError):rejected.append(key)
  else:raise AssertionError('accepted '+key)
 for key,value in [('production_enabled',True),('final_charge',True),('quota_refusal_preserves_graph_history_and_charge',False)]:
  altered=copy.deepcopy(native);altered[key]=value
  try:validate(original,altered)
  except (AssertionError,KeyError):rejected.append(key)
  else:raise AssertionError('accepted '+key)
 altered=copy.deepcopy(original);altered['states'].pop()
 try:validate(altered,native)
 except (AssertionError,KeyError):rejected.append('source inventory')
 else:raise AssertionError('accepted truncated original')
 print(json.dumps({'exact_states':18,'rejected_mutations':rejected,'production_enabled':False}))
if __name__=='__main__':main()
