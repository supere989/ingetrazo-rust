#!/usr/bin/env python3
"""Strict frozen import states, bytes and actual original-reader pairs."""
import copy,json,math,sys
from pathlib import Path
D='dwg'
def exact(a,b,p=''):
 assert type(a)is type(b),(p,'type')
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(p,'keys')
  for k in a:exact(a[k],b[k],p+'/'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(p,'length')
  for i,(x,y)in enumerate(zip(a,b)):exact(x,y,p+'/'+str(i))
 else:
  assert a==b,(p,a,b)
  if isinstance(a,float):
   assert math.isfinite(a)and math.isfinite(b)
   if a==0:assert math.copysign(1,a)==math.copysign(1,b),(p,'signedzero')
def file(root,name):
 p=(root/name).resolve();assert p.is_relative_to(root);return p
s=Path(sys.argv[1]).resolve();n=Path(sys.argv[2]).resolve();report=Path(sys.argv[3]);r={'ok':False}
try:
 a=json.loads((s/f'original-{D}.json').read_text());b=json.loads((n/f'native-{D}.json').read_text())
 names=[c['name']for c in a['cases']];assert names==['real-root','real-nested','unit-cancel','chooser-cancel']
 assert [v['name']for v in a['repairs']]==['null','duplicate-seed','control-crlf-tail']
 assert [v['name']for v in a['discovery']]==['vendor','sibling','PATH','missing','windows-exe','linux-no-exe']
 assert [v['name']for v in a['converter_controls']]==['warning-output','no-output','timeout']
 inventory=[(c['name'],p)for c in a['cases']for p in c['phases']]
 assert inventory==[(name,phase)for name in names for phase in (['before','import','undo','redo']if name.startswith('real-')else['before','import'])]
 assert inventory==[(v['case'],v['phase'])for v in a['states']]
 diagnostics={'camera':[], 'converter':[]}
 def semantic(v):
  v=copy.deepcopy(v)
  for row in v['states']:diagnostics['camera'].append(row.pop('camera'))
  for row in v.get('converter_controls',[]):
   for key in ['source_diagnostic','native_diagnostic']:
    if key in row:diagnostics['converter'].append(row.pop(key))
  return v
 expected=semantic(a);observed=semantic(b);exact(expected,observed)
 payloads=0
 for section,key in [('repairs','file'),('decodes','rgba'),('resources','file'),('resources','rgba')]:
  for v in a.get(section,[]):assert file(s,v[key]).read_bytes()==file(n,v[key]).read_bytes();payloads+=1
 readers=json.loads((n/f'reopens-{D}.json').read_text())
 assert [v['case']for v in readers]==[c['name']for c in a['cases']if c.get('saved')]
 for v in readers:exact(v['original'],v['native'])
 guards=json.loads((n/f'guards-{D}.json').read_text())
 required=['actual_ingress','atomic_failure','temporary_cleanup','successful_fit']if D=='dwg'else['actual_ingress','atomic_failure','chooser_cancel','streaming_admission']
 for k in required:assert guards.get(k)is True,('guard',k)
 probes=[]
 for k in ['undo','redo']:
  c=copy.deepcopy(b);c['states'][0][k]+=1;probes.append(c)
 c=copy.deepcopy(b);c['states'].pop();probes.append(c)
 c=copy.deepcopy(b);c['states'][0]['scene']={'corruption':True};probes.append(c)
 for c in probes:
  try:exact(expected,semantic(c))
  except AssertionError:pass
  else:raise AssertionError('negative accepted')
 r.update(diagnostic_scope='camera fit and process-specific argv diagnostics excluded; all model/control fields strict',ok=True,cases=len(names),states=len(inventory),readers=len(readers),payloads=payloads,negative_controls=4)
except Exception as e:r['error']=repr(e)
report.parent.mkdir(parents=True,exist_ok=True);report.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r));sys.exit(0 if r['ok']else 1)
