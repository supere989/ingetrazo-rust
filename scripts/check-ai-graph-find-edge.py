"""Exact degree-selected dynamic incidence lookup observations."""
import json,sys,copy
from pathlib import Path
def read(path):return json.loads(Path(path).read_text())
def validate(original,native):
 assert type(native)is dict and original.keys()==native.keys()
 assert type(native['undo'])is int and type(native['redo'])is int
 assert len(original['ordered_callbacks'])==17
 assert original==native
original,native=map(read,sys.argv[1:3]);validate(original,native)
rejected=[]
for name,changed in [('wrong-small-side',dict(native,ordered_callbacks=[x.replace('a-iter','b-iter')for x in native['ordered_callbacks']])),('lost-detached-hit',dict(native,ordered_callbacks=[x for x in native['ordered_callbacks']if x!='detached-public-list-hit'])),('stale-length-slot',dict(native,ordered_callbacks=[x.replace('new-other-len','old-other-len')for x in native['ordered_callbacks']])),('missing-dynamic-call',dict(native,ordered_callbacks=[x for x in native['ordered_callbacks']if x!='dynamic-other'])),('unexpected-history',dict(native,undo=2))]:
 try:validate(original,changed)
 except AssertionError:rejected.append(name)
 else:raise AssertionError('Mutation accepted: '+name)
print(json.dumps({'exact_ordered_observations':17,'negative_controls_rejected':rejected,'production_enabled':False}))
