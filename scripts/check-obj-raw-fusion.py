#!/usr/bin/env python3
import json,copy,sys
from pathlib import Path
p=Path(sys.argv[1]);a=json.loads((p/'original-obj-raw-fusion.json').read_text());b=json.loads((p/'native-obj-raw-fusion.json').read_text())
expected=['cross-cell-near-weld','same-cell-far-weld','textured-duplicate-winning-coordinate']
def strict(a,b,path=''):
 if type(a)is not type(b):assert type(a)in(int,float)and type(b)in(int,float),(path,'type')
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(path,'keys')
  for k in a:strict(a[k],b[k],path+'/'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,len(a),len(b))
  for i,(x,y)in enumerate(zip(a,b)):strict(x,y,path+'/'+str(i))
 else:assert a==b,(path,a,b)
assert [r['case']for r in a['cases']]==expected
assert [(r['case'],r['phase'])for r in a['states']]==[(n,phase)for n in expected for phase in ['before','import','undo','redo','finish']]
# Edge flags are an unordered inventory; both observers already sort them,
# but Python/serde scientific notation has different lexical ordering.
# Compare a numeric endpoint order without changing any endpoint/flag value.
def ordered_flags(doc):
 doc=copy.deepcopy(doc)
 def group(row):
  row['mesh'].sort(key=lambda r:tuple(x for end in r['ends']for x in end))
  for child in row['children']:group(child)
 for state in doc['states']:
  state['flags']['active'].sort(key=lambda r:tuple(x for end in r['ends']for x in end))
  for row in state['flags']['groups']:group(row)
 return doc
a=ordered_flags(a);b=ordered_flags(b)
strict(a,b)
reopens=json.loads((p/'reopens-obj-raw-fusion.json').read_text());assert [r['case']for r in reopens]==expected
for row in reopens:strict(row['original'],row['native'])
mutations=[]
for name in expected:
 c=copy.deepcopy(b);r=next(r for r in c['states']if r['case']==name and r['phase']=='import');r['scene']['groups'][-1]['children'][0]['mesh']['faces'].pop();mutations.append(c)
 c=copy.deepcopy(b);r=next(r for r in c['states']if r['case']==name and r['phase']=='import');r['undo']+=1;mutations.append(c)
c=copy.deepcopy(b);r=next(r for r in c['states']if r['case']==expected[-1]and r['phase']=='import');r['scene']['groups'][-1]['children'][0]['mesh']['points'][0][0]+=.000001;mutations.append(c)
for c in mutations:
 try:strict(a,c)
 except AssertionError:pass
 else:raise AssertionError('raw loop mutation accepted')
print(json.dumps({'cases':3,'states':15,'actual_original_reopens':3,'negative_controls':len(mutations),'passed':True}))
