#!/usr/bin/env python3
import copy,json,sys
from pathlib import Path
p=Path(sys.argv[1]);source=json.loads((p/'original.json').read_text());native=json.loads((p/'native.json').read_text())
def validate(value):
 assert isinstance(value,dict) and set(value)=={'states'}
 rows=value['states'];assert isinstance(rows,list) and len(rows)==42
 assert [(x['case'],x['phase'])for x in rows]==[(c,phase)for c in ['glyph','dimension','leader-miss','hidden','locked','nested']for phase in ['before','grab','preview','cancel','commit','undo','redo']]
 assert value==source,'Exact source state mismatch'
validate(native)
negatives=0
for i in range(42):
 mutated=copy.deepcopy(native);mutated['states'][i]['active']=not mutated['states'][i]['active']
 try:validate(mutated)
 except AssertionError:negatives+=1
 else:raise AssertionError('Mutation accepted')
assert (p/'source-first-failure.log').stat().st_size>0
try:validate(json.loads((p/'threshold-first-failure.json').read_text()))
except AssertionError:pass
else:raise AssertionError('Wrong float64 camera threshold candidate accepted')
print(json.dumps({'exact_states':42,'rejected_mutations':negatives,'rejected_retained_threshold_candidate':True},indent=2))
