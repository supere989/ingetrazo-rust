"""Focused graph comparison order and defensive publication admission, not full parity."""
import json,sys,copy
from pathlib import Path
def document(path):
 text=Path(path).read_text()
 return json.loads(next(row for row in reversed(text.splitlines()) if row.startswith('{')))
def validate_order(expected,actual):
 assert expected==actual
 assert actual['ordered_callbacks']==['s0','s1','h0','h1','l0','l1','c0','c1']
 assert type(actual['undo']) is int and actual['undo']==1 and actual['redo']==0
def validate_safety(actual):
 assert actual=={'identity_refusals':['created-edge-order','created-edge-replacement','created-edge-endpoint'],'real_caught_reentry_controls':2,'redo_preserved':True,'final_charge':0,'production_enabled':False,'source_parity_claim':False}
 assert type(actual['real_caught_reentry_controls']) is int
expected,actual,safety=map(document,sys.argv[1:4])
validate_order(expected,actual);validate_safety(safety)
rejected=[]
for name,validator,changed in [
 ('field-order',lambda x:validate_order(expected,x),dict(actual,ordered_callbacks=list(reversed(actual['ordered_callbacks'])))),
 ('missing-reentry',validate_safety,dict(safety,real_caught_reentry_controls=1)),
 ('lost-redo',validate_safety,dict(safety,redo_preserved=False)),
 ('retained-charge',validate_safety,dict(safety,final_charge=1)),
 ('false-completion',validate_safety,dict(safety,source_parity_claim=True))]:
 try:validator(changed)
 except AssertionError:rejected.append(name)
 else:raise AssertionError('Mutation accepted: '+name)
print(json.dumps({'ordered_source_callbacks_exact':8,'defensive_identity_controls':3,'caught_poison_reentry_controls':2,'negative_controls_rejected':rejected,'production_enabled':False}))
