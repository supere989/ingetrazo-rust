#!/usr/bin/env python3
"""Independent fan boundary integral and oriented edge inventory, no native volume helper."""
import json,sys,math,collections,copy
from pathlib import Path
def check(r):
 assert r['invalid_transform_atomic'] is True
 rows=r['cases'];assert len(rows)==9
 assert {(x['scenario'],x['operation']) for x in rows}=={(s,o) for s in ['overlap','disjoint','transformed'] for o in ['Union','Subtract','Intersect']}
 assert {x['scenario'] for x in rows}=={'overlap','disjoint','transformed'}
 for row in rows:
  if row['refused']:
   assert row['scenario']=='disjoint' and row['unchanged_refusal'] and not row['history_restored'];assert len(row['meshes'])==2;continue
  assert row['history_restored'] and len(row['meshes'])==1
  m=row['meshes'][0];assert m['transform']==[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
  v=m['vertices'];edges=collections.defaultdict(list);volume=0.;owners=set()
  assert v and m['faces'] and all(math.isfinite(c) for p in v for c in p)
  for f in m['faces']:
   assert not f['holes'];ids=f['outer'];assert len(ids)>=3 and all(0<=i<len(v) for i in ids)
   assert f['attrs']['future']=={'opaque':[1,2]};owners.add(f['attrs']['owner'])
   a=v[ids[0]]
   for i in range(1,len(ids)-1):
    b=v[ids[i]];c=v[ids[i+1]];volume+=sum(a[j]*(b[(j+1)%3]*c[(j+2)%3]-b[(j+2)%3]*c[(j+1)%3]) for j in range(3))/6
   for a,b in zip(ids,ids[1:]+ids[:1]):edges[tuple(sorted((a,b)))].append(1 if a<b else -1)
  assert all(len(x)==2 and sum(x)==0 for x in edges.values()),row
  assert abs(abs(volume)-row['expected_volume'])<1e-8,(row['scenario'],row['operation'],volume)
  assert owners and owners<={'a','b'}
 return len(rows)
r=json.loads(Path(sys.argv[1]).read_text());n=check(r)
# Literal independent-oracle corruptions must reject.
negative=0
for kind in ['volume','winding','attributes','history','inventory']:
 x=copy.deepcopy(r);row=next(q for q in x['cases'] if not q['refused'])
 if kind=='volume':row['expected_volume']+=.25
 elif kind=='winding':row['meshes'][0]['faces'][0]['outer'].reverse()
 elif kind=='attributes':row['meshes'][0]['faces'][0]['attrs']['future']={}
 elif kind=='history':row['history_restored']=False
 else:x['cases'].pop()
 try:check(x)
 except AssertionError:negative+=1
 else:raise AssertionError('negative accepted: '+kind)
print(json.dumps({'passed':True,'cases':n,'negative_controls':negative}))
