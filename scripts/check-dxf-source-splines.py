#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
p=Path(sys.argv[1]);a=json.loads((p/'original-dxf-splines.json').read_text());b=json.loads((p/'native-dxf-splines.json').read_text())
expected=['cubic-single','cubic-spans','knots-rounded','quadratic','quartic','rational-quadratic','rational-cubic','periodic','periodic-rational','knots-shifted','missing-knots','fit-natural','fit-tangents','closed-flag-open-path','invalid-weights-with-line']
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
r=json.loads((p/'reopens-dxf-splines.json').read_text());assert [x['case']for x in r]==expected
for x in r:strict(x['original'],x['native'])
guards=json.loads((p/'native-dxf-guards.json').read_text());assert set(guards)=={'cancel_preserves_document_history_revision','malformed_sequence_preserves_document_history_revision','native_f64_and_opaque_payload_preserved'};assert all(v is True for v in guards.values())
mutations=[]
for name in expected:
 c=copy.deepcopy(b);row=next(r for r in c['states']if r['case']==name and r['phase']=='import');row['scene']['groups'][-1]['mesh']['edges'].pop();mutations.append(c)
 c=copy.deepcopy(b);row=next(r for r in c['states']if r['case']==name and r['phase']=='import');row['undo']+=1;mutations.append(c)
 c=copy.deepcopy(b);row=next(r for r in c['states']if r['case']==name and r['phase']=='import');row['curves'][-1]['owners']+=1;mutations.append(c)
for c in mutations:
 try:strict(a,c)
 except AssertionError:pass
 else:raise AssertionError('DXF mutation accepted')
strict(json.loads((p/'original-spline-errors.json').read_text()),json.loads((p/'native-spline-errors.json').read_text()))
print(json.dumps({'cases':15,'states':105,'actual_original_reopens':15,'negative_controls':len(mutations),'ownership_guards':3,'passed':True}))
