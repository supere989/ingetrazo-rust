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
def winding_diff(a,b,path=''):
 if isinstance(a,dict):
  assert isinstance(b,dict)and a.keys()==b.keys(),(path,'keys')
  rows=[]
  for k in a:rows.extend(winding_diff(a[k],b[k],path+'/'+k))
  return rows
 if isinstance(a,list)and a and isinstance(a[0],dict)and 'outer'in a[0]and path.endswith('/faces'):
  assert isinstance(b,list)and len(a)==len(b),(path,'face count')
  def ring_key(r):
   assert r and all(isinstance(p,list)and all(type(c)in(int,float)for c in p)for p in r),(path,'ring type')
   t=tuple(tuple(p)for p in r);return min(t[i:]+t[:i]for i in range(len(t)))
  def geometry(f):return min(ring_key(f['outer']),ring_key(list(reversed(f['outer'])))),tuple(sorted(min(ring_key(h),ring_key(list(reversed(h))))for h in f['holes']))
  used=set();rows=[]
  for i,f in enumerate(a):
   hits=[j for j,g in enumerate(b)if j not in used and geometry(f)==geometry(g)]
   assert len(hits)==1,(path,'different/ambiguous face geometry');j=hits[0];used.add(j);g=b[j]
   strict({k:v for k,v in f.items()if k!='outer'},{k:v for k,v in g.items()if k!='outer'},path+'/'+str(i))
   if f['outer']!=g['outer']:
    assert ring_key(f['outer'])==ring_key(list(reversed(g['outer']))),(path,'non-winding difference')
    rows.append({'path':path+'/'+str(i)+'/outer','kind':'allocation-dependent-winding','native_face_index':j,'source':f['outer'],'native':g['outer']})
  return rows
 if isinstance(a,list):
  assert isinstance(b,list)and len(a)==len(b),(path,'list length');rows=[]
  for i,(x,y)in enumerate(zip(a,b)):rows.extend(winding_diff(x,y,path+'/'+str(i)))
  return rows
 strict(a,b,path);return []
known='closed-cube-leaf';full_diff=[]
for x,y in zip(a['states'],b['states']):
 if x['case']!=known:strict(x,y);continue
 try:strict(x,y)
 except AssertionError:
  assert x['phase']in ['import','redo','finish'],('unapproved phase',x['phase'])
  diff=winding_diff(x,y);assert diff,'unclassified strict difference'
  full_diff.extend({'case':known,'phase':x['phase'],**r}for r in diff)
assert {r['phase']for r in full_diff}=={'import','redo','finish'},'retained boundary missing/unexpected'
(p/'full-winding-diff.json').write_text(json.dumps(full_diff,indent=2)+'\n')
deterministic_a={**a,'cases':[r for r in a['cases']if r['case']!=known],'states':[r for r in a['states']if r['case']!=known]}
if '--deterministic'in sys.argv:
 deterministic_b={**b,'cases':[r for r in b['cases']if r['case']!=known],'states':[r for r in b['states']if r['case']!=known]}
 strict(deterministic_a,deterministic_b)
else:strict(a,b)
controls=json.loads((p/'native-fusion-controls.json').read_text());assert controls=={'native_f64_and_owner_metadata_preserved':True,'invalid_import_atomic':True}
mutations=[]
for case in [n for n in expected if '--deterministic'not in sys.argv or n!='closed-cube-leaf']:
 c=copy.deepcopy(b);row=next(r for r in c['states']if r['case']==case and r['phase']=='import');row['undo']+=1;mutations.append(c)
for case in ['explicit-smooth-root','reference-auto-smooth']:
 c=copy.deepcopy(b);row=next(r for r in c['states']if r['case']==case and r['phase']=='import');mesh=row['flags']['active'] if case=='explicit-smooth-root'else row['flags']['groups'][-1]['children'][0]['mesh'];edge=next(e for e in mesh if e['soft']);edge['soft']=False;mutations.append(c)
boundary_mutations=[]
src=next(r for r in a['states']if r['case']==known and r['phase']=='import')
base=next(r for r in b['states']if r['case']==known and r['phase']=='import')
for kind in ['history','paint','position']:
 candidate=copy.deepcopy(base)
 if kind=='history':candidate['undo']+=1
 elif kind=='paint':candidate['scene']['active']['faces'][0]['attrs']['opaque-loss']=True
 else:candidate['scene']['active']['faces'][0]['outer'][0][0]+=.125
 try:winding_diff(src,candidate)
 except AssertionError:boundary_mutations.append(kind)
 else:raise AssertionError('unrelated boundary regression accepted')
for c in mutations:
 try:
  if '--deterministic'in sys.argv:strict(deterministic_a,{**c,'cases':[r for r in c['cases']if r['case']!=known],'states':[r for r in c['states']if r['case']!=known]})
  else:strict(a,c)
 except AssertionError:pass
 else:raise AssertionError('mutation accepted')
reopens=json.loads((p/'reopens-obj-fusion.json').read_text());assert [r['case']for r in reopens]==expected
for row in reopens:
 if '--deterministic'in sys.argv and row['case']==known:
  diff=winding_diff(row['original'],row['native']);assert diff,'reopen boundary missing';full_diff.extend({'case':known,'phase':'reopen',**r}for r in diff);continue
 strict(row['original'],row['native'])
(p/'full-winding-diff.json').write_text(json.dumps(full_diff,indent=2)+'\n')
print(json.dumps({'cases':11 if '--deterministic'in sys.argv else len(a['cases']),'states':55 if '--deterministic'in sys.argv else len(a['states']),'unresolved_source_winding_case':'closed-cube-leaf' if '--deterministic'in sys.argv else None,'unresolved_winding_leaves':len(full_diff),'negative_controls':len(mutations)+len(boundary_mutations),'ownership_controls':2,'passed':True}))
