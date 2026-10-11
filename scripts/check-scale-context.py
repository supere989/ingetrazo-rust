#!/usr/bin/env python3
"""Literal parity inventory plus separately checked native integrity boundaries."""
import json,sys,copy
NAMES=['nested-preview','nested-commit','image-preview','mixed-preview','entry-preview']
PHASES=[['before','preview','boundary','tool-cancel'],['before','preview','commit','boundary','retype','undo','redo'],['before','preview','boundary','tool-cancel'],['before','preview','boundary','tool-cancel'],['before','preview','boundary','tool-cancel']]
EXPECTED={('nested-preview','boundary'):['active','undo','context_groups'],('nested-preview','tool-cancel'):['undo','context_groups'],('mixed-preview','boundary'):['images','active','undo','context_groups'],('mixed-preview','tool-cancel'):['undo','context_groups'],('entry-preview','boundary'):['active','context_groups'],('entry-preview','tool-cancel'):['context_groups']}
FIELDS=['mesh','images','depth','active','undo','redo','context_groups']
def context(s):
 d=s['depth'];return s['groups']if d==0 else s['groups'][0]['children']if d==1 else []
def world(g,parent=None):
 # Integer translations in this fixed corpus; includes corruption detector.
 m=g['transform'];m=m if m is not None else [1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
 if parent is None:parent=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
 product=[sum(parent[k*4+r]*m[c*4+k]for k in range(4))for c in range(4)for r in range(4)]
 out=[[sum(product[k*4+r]*p[k]for k in range(3))+product[12+r]for r in range(3)]for p in g['vertices']]
 for child in g['children']:out+=world(child,product)
 return out
OLD=[[5,7,0],[7,7,0],[7,10,0],[5,10,0]]
def validate(source,native):
 assert source['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [r['name']for r in source['cases']]==NAMES==[r['name']for r in native['cases']]
 rows=[];exact=0
 for a,b,phases in zip(source['cases'],native['cases'],PHASES):
  assert list(a['states'])==phases
  assert list(b['states'])==phases+['context-cancel']
  for phase in phases:
   s=a['states'][phase];n=b['states'][phase];s=dict(s,context_groups=context(s))
   diff=[key for key in FIELDS if s[key]!=n[key]]
   assert diff==EXPECTED.get((a['name'],phase),[]),(a['name'],phase,diff)
   exact+=not diff;rows.append({'case':a['name'],'phase':phase,'literal_exact':not diff,'different_fields':diff})
  # Native Cancel returns the outer document exactly, not stale inner owners.
  cancelled=b['states']['context-cancel'];assert cancelled['depth']==0 and not cancelled['active'] and cancelled['undo']==0
  assert cancelled['mesh']==[];assert len(cancelled['context_groups'])==1
  assert world(cancelled['context_groups'][0])==OLD
  assert cancelled['images']==[[[1.,2.,0.],[2.,0.,0.],[0.,3.,0.]]]
  if a['name']in ['nested-preview','mixed-preview','entry-preview']:
   for phase in ['boundary','tool-cancel']:
    n=b['states'][phase];assert not n['active'] and n['undo']==0
    assert n['mesh']==[];assert len(n['context_groups'])==1;assert world(n['context_groups'][0])==OLD
    assert n['images']==cancelled['images']
 assert exact==17
 return {'literal_source_parity':False,'exact_phases':exact,'total_source_phases':23,'retained_divergent_phases':6,'native_cancel_invariants':5,'phases':rows}
s=json.load(open(sys.argv[1]));n=json.load(open(sys.argv[2]));report=validate(s,n)
mutations=[]
a=copy.deepcopy(n);a['cases']=[];mutations.append(a)
a=copy.deepcopy(n);a['cases'][1]['states']['redo']['context_groups'][0]['vertices'][1][0]+=.25;mutations.append(a)
a=copy.deepcopy(n);a['cases'][2]['states']['tool-cancel']['images'][0][1][0]=4.;mutations.append(a)
a=copy.deepcopy(n);a['cases'][4]['states']['context-cancel']['context_groups'][0]['transform'][12]+=1.;mutations.append(a)
a=copy.deepcopy(n);a['cases'][0]['states']['boundary']['undo']=1;mutations.append(a)
for a in mutations:
 try:validate(s,a)
 except AssertionError:continue
 raise AssertionError('corruption accepted')
report['rejected_mutations']=len(mutations);print(json.dumps(report,indent=2))
