#!/usr/bin/env python3
"""Exact state, real reader, sharing/placement and atomic ownership controls."""
import copy,json,sys
from pathlib import Path
p=Path(sys.argv[1]);a=json.loads((p/'original-dxf-blocks.json').read_text());b=json.loads((p/'native-dxf-blocks.json').read_text())
expected=['shared-base','shared-leaf','unit-centimetres','case-sensitive-prototypes','mixed-order','nested-lines','nested-curves','oblique-insert','minsert-first-placement','missing-annotation-mixed','utm-recenter','depth-limit','block-face']
def strict(a,b,path=''):
 if type(a)is not type(b):assert type(a)in(int,float)and type(b)in(int,float),(path,'type')
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(path,'keys')
  for k in a:strict(a[k],b[k],path+'/'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,len(a),len(b))
  for i,(x,y)in enumerate(zip(a,b)):strict(x,y,path+'/'+str(i))
 else:assert a==b,(path,a,b)
assert [r['name']for r in a['cases']]==expected
inventory=[(n,phase)for n in expected for phase in ['before','import','undo','redo','finish','finish-undo','finish-redo']]
assert [(r['case'],r['phase'])for r in a['states']]==inventory
strict(a,b)
r=json.loads((p/'reopens-dxf-blocks.json').read_text());assert [x['case']for x in r]==expected
for x in r:strict(x['original'],x['native'])
g=json.loads((p/'native-dxf-guards.json').read_text());assert all(v is True for v in g.values()) and len(g)==3
cancel=json.loads((p/'cancel-block-reopen.json').read_text());strict(cancel['original'],cancel['native'])
assert json.loads((p/'native-block-cancel.json').read_text())=={'cancel_keeps_root_import_sharing_and_registries':True}
mutations=[]
for name in expected:
 for key in ['geometry','placement','history','owner']:
  c=copy.deepcopy(b);row=next(x for x in c['states']if x['case']==name and x['phase']=='import')
  if key=='geometry':row['scene']['groups'][-1]['mesh']['edges'].pop()
  elif key=='placement':row['owners']['groups'][-1]['xform'][12]+=1
  elif key=='history':row['undo']+=1
  else:row['owners']['groups'][-1]['component']=not row['owners']['groups'][-1]['component']
  mutations.append(c)
for c in mutations:
 try:strict(a,c)
 except AssertionError:pass
 else:raise AssertionError('DXF block mutation accepted')
sa=json.loads((p/'original-dxf-blocks-addendum-all.json').read_text());sb=json.loads((p/'native-dxf-blocks-addendum.json').read_text())
extra=['uniform-circle','uniform-arc','attributed-insert','anisotropic-lw-bulge','anisotropic-legacy-bulge','unit-nested-spline','uniform-full-arc']
assert [c['name']for c in sa['cases']]==extra
assert [(r['case'],r['phase'])for r in sa['states']]==[(n,phase)for n in extra for phase in ['before','import','undo','redo','finish','finish-undo','finish-redo']]
strict(sa,sb)
r=json.loads((p/'reopens-dxf-blocks-addendum.json').read_text());assert [x['case']for x in r]==extra
for x in r:strict(x['original'],x['native'])
for name in extra:
 for key in ['geometry','placement','history','owner']:
  c=copy.deepcopy(sb);row=next(x for x in c['states']if x['case']==name and x['phase']=='import')
  if key=='geometry':row['scene']['groups'][-1]['mesh']['edges'].pop()
  elif key=='placement':row['owners']['groups'][-1]['xform'][12]+=1
  elif key=='history':row['undo']+=1
  else:row['owners']['groups'][-1]['component']=not row['owners']['groups'][-1]['component']
  try:strict(sa,c)
  except AssertionError:pass
  else:raise AssertionError('addendum mutation accepted')
print(json.dumps(dict(cases=20,states=140,actual_original_reopens=20,cancel_reopen=1,negative_controls=len(mutations)+28,ownership_guards=4,passed=True)))
