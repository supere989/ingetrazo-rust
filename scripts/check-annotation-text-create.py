#!/usr/bin/env python3
"""Strict actual Text handler/App prompt/history comparison and rejecting mutations."""
import copy,json,sys
from pathlib import Path
p=Path(sys.argv[1]);source=json.loads((p/'original.json').read_text());native=json.loads((p/'native.json').read_text())
names=[c+'-'+k for c in ['root','nested'] for k in ['edge','face','coordinates']]+['closed-face']
phases=['before','anchor','pending-cancel','cancel','pending-accept','commit','undo','redo','pending-accept','empty','finish']
def validate(value):
 rows=value['states'];assert len(rows)==77
 assert [(r['case'],r['phase']) for r in rows]==[(n,q) for n in names for q in phases]
 assert rows==source['states'],'literal source Text prompt/history mismatch'
validate(native)
rejected=0
for key in ['suggestion','anchor','selection','labels','undo','redo','depth']:
 candidate=copy.deepcopy(native);r=candidate['states'][5]
 r[key]={"mutated":True}
 try:validate(candidate)
 except (AssertionError,KeyError,TypeError):rejected+=1
 else:raise AssertionError('checker accepted altered '+key)
for index in [1,2,3,6,7,8,10]:
 candidate=copy.deepcopy(native);candidate['states'][index]['case']='wrong'
 try:validate(candidate)
 except AssertionError:rejected+=1
 else:raise AssertionError('checker accepted altered observation order')
print(json.dumps(dict(exact_states=77,rejected_mutations=rejected)))

source_history=json.loads((p/'original-history.json').read_text())
history=json.loads((p/'native-history.json').read_text())
def validate_history(value):
 assert [r['phase'] for r in value['states']]==['created','entered-selected','undo','redo','tool-cancel','finish']
 assert value==source_history,'ancestor command/selection literal source mismatch'
validate_history(history)
for key in ['edge_selection','label_selection','vertices','undo','redo','depth']:
 candidate=copy.deepcopy(history);candidate['states'][2][key]=None
 try:validate_history(candidate)
 except AssertionError:pass
 else:raise AssertionError('accepted altered ancestor '+key)
print(json.dumps(dict(exact_ancestor_states=6,rejected_ancestor_mutations=6)))

def validate_payload(path):
 raw=json.loads(path.read_text())['scene']
 assert raw['units']['vendor']=={'keep':[1,{'opaque':True}]}
 assert raw['text_labels'][0]['vendor']=={'keep':['authored',7]}
for name in names:validate_payload(p/(name+'-native.igz'))
failed=p/'failed-unknown-label-root-edge.igz'
assert failed.is_file(),'mandatory first payload-loss candidate absent'
try:validate_payload(failed)
except (AssertionError,KeyError):pass
else:raise AssertionError('accepted retained original payload-loss failure')
print(json.dumps(dict(exact_payload_reopens=7,rejected_retained_loss=1)))

reopens=json.loads((p/'reopen.json').read_text())
def validate_reopens(value):
 assert [r['case']for r in value['cases']]==names
 for row in value['cases']:assert row['original']==row['native'],'actual original reopen differs '+row['case']
validate_reopens(reopens)
for index in [0,3,6]:
 candidate=copy.deepcopy(reopens);candidate['cases'][index]['native']['labels']=[]
 try:validate_reopens(candidate)
 except AssertionError:pass
 else:raise AssertionError('accepted altered original reopen')
print(json.dumps(dict(actual_original_reopens=7,rejected_reopen_mutations=3)))

failed_suggestion=json.loads((p/'failed-group-edge-suggestion.json').read_text())
try:validate(failed_suggestion)
except AssertionError:pass
else:raise AssertionError('accepted retained grouped-edge suggestion error')
print(json.dumps(dict(rejected_retained_suggestion=1)))
