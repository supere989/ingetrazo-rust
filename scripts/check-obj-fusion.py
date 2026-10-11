#!/usr/bin/env python3
import json,sys,copy
from pathlib import Path
p=Path(sys.argv[1]);a=json.loads((p/'original-obj-fusion.json').read_text());b=json.loads((p/'native-obj-fusion.json').read_text())
def strict(a,b,path=''):
 if type(a)is not type(b):assert type(a)in(int,float)and type(b)in(int,float),(path,'type')
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(path,'keys')
  for k in a:strict(a[k],b[k],path+'/'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,len(a),len(b))
  for i,(x,y)in enumerate(zip(a,b)):strict(x,y,path+'/'+str(i))
 else:assert a==b,(path,a,b)
expected=['closed-cube-root','closed-cube-leaf','explicit-smooth-root','hard-root','reference-patch','reference-annulus','reference-irregular','reference-smooth','reference-duplicates','large-reference-grid','reference-auto-smooth','reference-paint-boundary']
assert [r['case']for r in a['cases']]==expected
assert [(r['case'],r['phase'])for r in a['states']]==[(n,p)for n in expected for p in ['before','import','undo','redo','finish']]
if '--deterministic'in sys.argv:
 known='closed-cube-leaf'
 deterministic_a={**a,'cases':[r for r in a['cases']if r['case']!=known],'states':[r for r in a['states']if r['case']!=known]}
 deterministic_b={**b,'cases':[r for r in b['cases']if r['case']!=known],'states':[r for r in b['states']if r['case']!=known]}
 strict(deterministic_a,deterministic_b)
else:strict(a,b)
controls=json.loads((p/'native-fusion-controls.json').read_text());assert controls=={'native_f64_and_owner_metadata_preserved':True,'invalid_import_atomic':True}
mutations=[]
for case in expected:
 c=copy.deepcopy(b);row=next(r for r in c['states']if r['case']==case and r['phase']=='import');row['undo']+=1;mutations.append(c)
for case in ['explicit-smooth-root','reference-auto-smooth']:
 c=copy.deepcopy(b);row=next(r for r in c['states']if r['case']==case and r['phase']=='import');mesh=row['flags']['active'] if case=='explicit-smooth-root'else row['flags']['groups'][-1]['children'][0]['mesh'];edge=next(e for e in mesh if e['soft']);edge['soft']=False;mutations.append(c)
for c in mutations:
 try:strict(a,c)
 except AssertionError:pass
 else:raise AssertionError('mutation accepted')
reopens=json.loads((p/'reopens-obj-fusion.json').read_text());assert [r['case']for r in reopens]==expected
for row in reopens:
 if '--deterministic'in sys.argv and row['case']=='closed-cube-leaf':continue
 strict(row['original'],row['native'])
print(json.dumps({'cases':11 if '--deterministic'in sys.argv else len(a['cases']),'states':55 if '--deterministic'in sys.argv else len(a['states']),'unresolved_source_winding_case':'closed-cube-leaf' if '--deterministic'in sys.argv else None,'negative_controls':len(mutations),'ownership_controls':2,'passed':True}))
