"""Exact genuine Vertex/registry history validation and rejecting controls."""
import copy,json,sys,runpy
from pathlib import Path
exact=runpy.run_path(str(Path(__file__).with_name('check-ai-edge-replacement.py')))['exact']
def validate(source,native):
 exact(source['states'],native['states'])
 if len(source['states'])!=29:raise AssertionError('required eight cases and chronological states')
 exact(False,source['production_enabled']);exact(False,native['production_enabled'])
 exact(True,source['opaque_registry_payload_identity_history']);exact(True,native['opaque_registry_payload_identity_history'])
 exact(True,native['quota_refusal_preserves_graph_redo_and_charge']);exact(0,native['final_charge'])
 exact(set(native),{'states','final_charge','production_enabled','opaque_registry_payload_identity_history','quota_refusal_preserves_graph_redo_and_charge'})
def main():
 source=json.load(open(sys.argv[1]));native=json.load(open(sys.argv[2]));validate(source,native);rejected=[]
 for key in ['public','positions','edges','faces','registry','native_edges','native_faces','native_positions','undo','redo']:
  altered=copy.deepcopy(native);altered['states'][0][key]=None
  try:validate(source,altered)
  except (AssertionError,KeyError):rejected.append(key)
  else:raise AssertionError('accepted '+key)
 for key,value in [('final_charge',True),('production_enabled',True),('opaque_registry_payload_identity_history',False),('quota_refusal_preserves_graph_redo_and_charge',False)]:
  altered=copy.deepcopy(native);altered[key]=value
  try:validate(source,altered)
  except (AssertionError,KeyError):rejected.append(key)
  else:raise AssertionError('accepted '+key)
 altered=copy.deepcopy(source);altered['states'].pop()
 try:validate(altered,native)
 except (AssertionError,KeyError):rejected.append('source inventory')
 else:raise AssertionError('accepted truncated source')
 print(json.dumps({'exact_states':29,'rejected_mutations':rejected,'production_enabled':False}))
if __name__=='__main__':main()
