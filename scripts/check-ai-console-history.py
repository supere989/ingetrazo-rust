"""Strict same-validator source equality and executable negative controls."""
import copy,json,sys
from pathlib import Path
def validate(source,native):
 names=[r['name']for r in json.loads((Path(__file__).resolve().parents[1]/'docs/audits/ai-console-history/recipes.json').read_text())]
 assert [s['name']for s in source['states']]==names
 assert [s['name']for s in native['states']]==names
 assert native['states']==source['states']
 assert native['metrics']==dict(native_exports=3,vm_history_steps=7,final_owner_charge=0)
source=json.loads(Path(sys.argv[1]).read_text());native=json.loads(Path(sys.argv[2]).read_text());validate(source,native)
mutations=[]
for label,key,value in [('position','positions',[[99,0,0],[1,0,0],[0,1,0]]),('attrs','attrs',[{'mat':'Wrong'}]),('undo','undo',99),('redo','redo',99),('identity','identity_checked',False)]:
 bad=copy.deepcopy(native);bad['states'][3][key]=value;mutations.append((label,bad))
bad=copy.deepcopy(native);bad['states'].pop();mutations.append(('missing',bad))
bad=copy.deepcopy(native);bad['states'][0],bad['states'][1]=bad['states'][1],bad['states'][0];mutations.append(('order',bad))
for key in native['metrics']:
 bad=copy.deepcopy(native);bad['metrics'][key]+=1;mutations.append((key,bad))
rejected=[]
for label,bad in mutations:
 try:validate(source,bad)
 except (AssertionError,KeyError,TypeError):rejected.append(label)
 else:raise AssertionError('Accepted mutation '+label)
print(json.dumps(dict(states=len(native['states']),rejected=rejected),indent=2))
