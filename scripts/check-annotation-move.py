#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
p=Path(sys.argv[1]);a=json.loads((p/'original.json').read_text());b=json.loads((p/'native.json').read_text())
def validate(x):
 assert len(x['states'])==len(a['states'])
 for i,(u,v)in enumerate(zip(a['states'],x['states'])):assert u==v,(i,u,v)
validate(b)
for i in range(len(b['states'])):
 bad=copy.deepcopy(b);bad['states'][i]['selection']+=1
 try:validate(bad)
 except AssertionError:pass
 else:raise AssertionError('mutation accepted')
old_a=json.loads((p/'annotation-move-original-first-failure.json').read_text())['states']
old_b=json.loads((p/'annotation-move-native-first-failure.json').read_text())['states']
assert any(x!=y for x,y in zip(old_a,old_b)), 'missing retained failure'
boundaries=json.loads((p/'boundaries.json').read_text());assert [x['kind']for x in boundaries]==['oblique','collapsed']
oblique=boundaries[0]['observations']
for native in oblique:
 source=next(x for x in old_a if x['case']=='nested' and x['phase']==native['phase'])
 assert native['labels']==source['labels'] and native['dimensions'][0]==source['dimensions'][0], 'oblique annotation divergence'
 assert native['vertices']==oblique[0]['vertices'], 'oblique native geometry precision lost'
collapsed=boundaries[1]['observations'];assert [x['phase'] for x in collapsed]==['before','preview','commit']
assert collapsed[-1]['vertices']==collapsed[0]['vertices'];assert collapsed[-1]['undo']==collapsed[0]['undo'];assert boundaries[1]['geometry_preserved']
# Selected free dimension endpoints are integral; this fixture's integral
# preview/Cancel delta restores them exactly, as the source mixed control shows.
post_cancel=next(x for x in old_a if x['case']=='mixed' and x['phase']=='cancel')
assert collapsed[-1]['dimensions']==collapsed[0]['dimensions'], 'collapsed refusal lost dimensions'
assert collapsed[-1]['dimensions'][0]==post_cancel['dimensions'][0]
# Label Cancel accumulates source f32 drift, so compare the retained source
# Cancel offsets instead of incorrectly requiring the initial offsets.
assert collapsed[-1]['labels']==post_cancel['labels'], 'collapsed refusal lost labels'
print(json.dumps({'mandatory_boundaries':2,'oblique_annotation_observations':len(oblique)}))
print(json.dumps({'states' :len(b['states']),'negative_controls':len(b['states']),'exact':True}))
