import json,copy,sys
from pathlib import Path
p=Path(sys.argv[1]);a=json.loads((p/'original.json').read_text());b=json.loads((p/'native.json').read_text())
names=['append','undo','noop-with-redo','failure-with-redo','redo','repeat-undo-0','repeat-redo-0','repeat-undo-1','repeat-redo-1','branch-undo','branch-new','branch-redo-refused']
def validate(x):
 assert [c['name']for c in x]==names
 assert x==a
validate(b);bad=[]
for name in ['orphan-on-snapshot-undo','lost-redo-on-noop','partial-failure-published','branch-redo-retained']:
 x=copy.deepcopy(b)
 if name=='orphan-on-snapshot-undo':x[1]['vertices']=[[0,0,0],[1,0,0]]
 elif name=='lost-redo-on-noop':x[2]['redo']=0
 elif name=='partial-failure-published':x[3]['vertices']=[[8,0,0],[9,0,0]]
 else:x[-1]['redo']=1
 try:validate(x)
 except AssertionError:bad.append(name)
 else:raise AssertionError('mutation accepted '+name)
print(json.dumps(dict(states=12,exact=True,negative_rejections=bad,scope='append-only topology/history, no VM deadline'),indent=2))
