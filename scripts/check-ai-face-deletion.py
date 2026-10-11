"""Strict operative Face deletion checker; every negative invokes the same validator."""
import copy,json,sys,runpy
from pathlib import Path
exact=runpy.run_path(str(Path(__file__).with_name('check-ai-edge-replacement.py')))['exact']
def validate(source,native):
 exact(source['states'],native['states'])
 exact(False,native['production_enabled']);exact(False,source['production_enabled'])
 exact(True,native['quota_refusal_preserves_graph_history_and_charge']);exact(0,native['final_charge'])
 exact(set(native),{'states','production_enabled','quota_refusal_preserves_graph_history_and_charge','final_charge'})
 if len(source['states'])!=31:raise AssertionError('required nine cases plus chronology')
def main():
 source=json.load(open(sys.argv[1]));native=json.load(open(sys.argv[2]));validate(source,native);rejected=[]
 for key in ['radial','native_radial','faces','native_faces','held_face_live','held_face_loop','held_edge_incidence','native_incidence','undo','redo']:
  modified=copy.deepcopy(native);modified['states'][0][key]=None
  try:validate(source,modified)
  except (AssertionError,KeyError):rejected.append(key)
  else:raise AssertionError('accepted '+key)
 for key,val in [('production_enabled',True),('final_charge',True),('quota_refusal_preserves_graph_history_and_charge',False)]:
  modified=copy.deepcopy(native);modified[key]=val
  try:validate(source,modified)
  except (AssertionError,KeyError):rejected.append(key)
  else:raise AssertionError('accepted '+key)
 modified=copy.deepcopy(source);modified['states'].pop()
 try:validate(modified,native)
 except (AssertionError,KeyError):rejected.append('source inventory')
 else:raise AssertionError('accepted truncated source')
 print(json.dumps({'exact_states':31,'rejected_mutations':rejected,'production_enabled':False}))
if __name__=='__main__':main()
