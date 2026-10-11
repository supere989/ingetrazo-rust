"""Exact observable identity graph; packed native rows are keyed by actual UID owner."""
import copy,json,runpy,sys
from pathlib import Path
exact=runpy.run_path(str(Path(__file__).with_name('check-ai-edge-replacement.py')))['exact']
def validate(source,native):
 exact(set(source),{'states','production_enabled'});exact(set(native),{'states','final_charge','production_enabled','undo_redo_refusals_preserve_graph_history_charge'})
 if len(source['states'])!=35:raise AssertionError('seven complete repeated history cases required')
 a=copy.deepcopy(source['states']);b=copy.deepcopy(native['states'])
 for state in a+b:
  rows=state['native_positions']
  ids=[row['identity']for row in rows]
  if any(type(i)is not int for i in ids)or len(ids)!=len(set(ids)):raise AssertionError('actual packed identity missing or duplicated')
  state['native_positions']=sorted(rows,key=lambda row:row['identity'])
 exact(a,b);exact(False,source['production_enabled']);exact(False,native['production_enabled']);exact(0,native['final_charge']);exact(True,native['undo_redo_refusals_preserve_graph_history_charge'])
def main():
 source=json.load(open(sys.argv[1]));native=json.load(open(sys.argv[2]));validate(source,native);rejected=[]
 for key in ['public','positions','edges','faces','registry','native_edges','native_faces','undo','redo']:
  x=copy.deepcopy(native);x['states'][0][key]=None
  try:validate(source,x)
  except (AssertionError,KeyError,TypeError):rejected.append(key)
  else:raise AssertionError('accepted '+key)
 for key,value in [('identity',None),('position',[99,99,99])]:
  x=copy.deepcopy(native);x['states'][0]['native_positions'][0][key]=value
  try:validate(source,x)
  except (AssertionError,KeyError,TypeError):rejected.append('packed '+key)
  else:raise AssertionError('accepted packed '+key)
 for key,value in [('final_charge',True),('production_enabled',True),('undo_redo_refusals_preserve_graph_history_charge',False)]:
  x=copy.deepcopy(native);x[key]=value
  try:validate(source,x)
  except (AssertionError,KeyError):rejected.append(key)
  else:raise AssertionError('accepted '+key)
 x=copy.deepcopy(source);x['states'].pop()
 try:validate(x,native)
 except (AssertionError,KeyError):rejected.append('source inventory')
 else:raise AssertionError('accepted incomplete source')
 print(json.dumps({'exact_states':35,'rejected_mutations':rejected,'production_enabled':False}))
if __name__=='__main__':main()
