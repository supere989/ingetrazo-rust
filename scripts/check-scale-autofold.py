# SPDX-License-Identifier: GPL-3.0-or-later
import json,sys,copy
from pathlib import Path
from shapely.geometry import Polygon
from shapely.ops import unary_union
NAMES=['lifted-quad','gentle-hex','warped-hole','planar-hole']
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text())
def ring(r):return min(tuple(tuple(p)for p in r[i:]+r[:i])for i in range(len(r)))
def fields(fs):return sorted((ring(f['vertices']),tuple(sorted(ring(h)for h in f['holes'])),json.dumps(f['attrs'],sort_keys=True),f['interior'])for f in fs)
def coverage(fs):return unary_union([Polygon([(p[0],p[1])for p in f['vertices']],[[(p[0],p[1])for p in h]for h in f['holes']])for f in fs])
def check(v):
 assert s['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [c['name']for c in s['cases']]==NAMES==[c['name']for c in v['cases']]
 for a,b in zip(s['cases'],v['cases'],strict=True):
  assert b['folded']==a['folded'] and b['untouched_identity'] and b['original_edges_survive']
  assert len(a['result'])==len(b['result'])
  if a['name']!='warped-hole':assert fields(a['result'])==fields(b['result'])
  else:
   assert coverage(a['result']).symmetric_difference(coverage(b['result'])).area==0
   assert fields([f for f in a['result']if any(p[2]!=0 for p in f['vertices'])])==fields([f for f in b['result']if any(p[2]!=0 for p in f['vertices'])])
   assert all(f['attrs']=={'color':[.2,.4,.6],'vendor':71}and not f['interior'] for f in b['result']if not f['attrs'].get('untouched'))
check(n);neg=[]
for name in ['empty','lost_fold','lost_metadata','wrong_lift','hole_fill','replaced_identity']:
 v=copy.deepcopy(n)
 if name=='empty':v['cases']=[]
 elif name=='lost_fold':v['cases'][0]['result']=copy.deepcopy(s['cases'][0]['source'])
 elif name=='lost_metadata':v['cases'][0]['result'][-1]['attrs']={}
 elif name=='wrong_lift':v['cases'][0]['result'][-1]['vertices'][0][2]+=1
 elif name=='hole_fill':v['cases'][2]['result'].append({'vertices':[[2,2,0],[4,2,0],[4,4,0],[2,4,0]],'holes':[],'attrs':{},'interior':False})
 else:v['cases'][0]['untouched_identity']=False
 try:check(v)
 except AssertionError:neg.append(name)
 else:raise AssertionError('accepted '+name)
Path(sys.argv[3]).write_text(json.dumps({'exact_cases':3,'warped_hole':'Exact lifted pieces, count, metadata and projected coverage; planar partition order is original set-identity nondeterministic','negative_controls':neg},indent=2)+'\n')
