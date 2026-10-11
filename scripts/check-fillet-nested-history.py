#!/usr/bin/env python3
"""Strict original nested lifecycle; historical model failure remains separate."""
import json,sys,copy,importlib.util,hashlib
from pathlib import Path
spec=importlib.util.spec_from_file_location("prior",Path(__file__).with_name("check-fillet-scoped.py"));prior=importlib.util.module_from_spec(spec);spec.loader.exec_module(prior)
r=Path(sys.argv[1]);raw=(r/'source-app.json').read_bytes();assert hashlib.sha256(raw).hexdigest()=='3f975fa3fba86a1278534723e92c78f6cb7acfb46a000bda42790dd4b0971ce8'
s=json.loads(raw);n=json.loads((r/'native.json').read_bytes())
def check(native):
 assert [x['name']for x in s['apps']]==[x['name']for x in native['apps']]
 count=0
 for a,b in zip(s['apps'],native['apps']):
  assert set(a['states'])==set(b['states'])
  for phase,p in a['states'].items():
   q=b['states'][phase];prior.check_mesh(p['mesh'],q['mesh'],False)
   for key in ['undo','redo','selection','preview_faces','preview_lines']:assert p[key]==q[key],(a['name'],phase,key)
   count+=1
 return count
count=check(n)
old=json.loads((r/'old-native.json').read_bytes())
try:check(old)
except (AssertionError,KeyError):pass
else:raise AssertionError('old candidate accepted')
for kind in ['orphan','curve','winding','history']:
 a=copy.deepcopy(n);st=a['apps'][-1]['states']['finish-redo'];m=st['mesh']
 if kind=='orphan':m['vertices'].append([99,99,99])
 elif kind=='curve':
  for e in m['edges']:
   if e['curve']is not None:e['curve']=0
 elif kind=='winding':m['faces'][0]['loop'].reverse()
 else:st['undo']+=1
 try:check(a)
 except (AssertionError,KeyError):pass
 else:raise AssertionError('mutation accepted '+kind)
print(json.dumps({'exact_app_states':count,'old_candidate_rejected':True,'negative_controls':4}))
