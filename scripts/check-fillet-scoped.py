#!/usr/bin/env python3
"""Exact inventory/ownership, oriented geometric matching at existing 1e-6 source boundary."""
import json,sys,copy,math,hashlib,zipfile
from pathlib import Path
NAMES=['endpoint-free-wire','open-neighbor','paint-first-rim','oblique-cap','concave','turned-squashed']
PHASES=['activate','segments','sizing','refusal','cancel','commit','undo','redo']
def check_mesh(s,n,origins=True,retained_orphans=False,curve_partition=True):
 for k in ['edges','faces']+([]if retained_orphans else ['vertices']):assert len(s[k])==len(n[k]),(k,len(s[k]),len(n[k]))
 mapping=[]
 for p in n['vertices']:
  ds=[sum((a-b)**2 for a,b in zip(p,q))for q in s['vertices']];i=min(range(len(ds)),key=ds.__getitem__)
  if ds[i]>=1e-12 and retained_orphans:
   assert list(p) in retained_orphans,'extra orphan is not an original rounded endpoint'
   vid=len(mapping);assert all(vid not in e['ends']for e in n['edges']) and all(vid not in f['loop'] and all(vid not in h for h in f['holes'])for f in n['faces']),'extra vertex is not an orphan';mapping.append(-vid-1)
  else:assert ds[i]<1e-12,('coordinate',p,s['vertices'][i]);mapping.append(i)
 assert len(set(mapping))==len(mapping),'welded distinct source vertices'
 if origins:assert s['vertex_origins']==n['vertex_origins'],'old vertex ownership/order'
 def cycle(r):return min(tuple(r[i:]+r[:i])for i in range(len(r)))
 def face(f,m):return (cycle([m[i]for i in f['loop']]),tuple(sorted(cycle([m[i]for i in h])for h in f['holes'])))
 fs={face(f,list(range(len(s['vertices'])))):f for f in s['faces']};assert len(fs)==len(s['faces'])
 for f in n['faces']:
  q=fs[face(f,mapping)]
  for k in ['attrs','interior']+(['origin']if origins else []):assert q[k]==f[k],('face',k,q[k],f[k])
 es={tuple(sorted(e['ends'])):e for e in s['edges']};assert len(es)==len(s['edges']);curves={};reverse={}
 for e in n['edges']:
  q=es[tuple(sorted(mapping[i]for i in e['ends']))]
  for k in ['attrs','soft','hidden','layer']+(['origin']if origins else []):assert q[k]==e[k],('edge',k,q[k],e[k])
  a,b=q['curve'],e['curve'];assert (a is None)==(b is None)
  if a is not None and curve_partition:
   assert curves.setdefault(a,b)==b and reverse.setdefault(b,a)==a,'curve partition'
   if origins and q['origin']is not None:assert a==b,'surviving authored curve ID'
 return True
def read(root,name):
 if (root/name).is_file():return (root/name).read_bytes()
 with zipfile.ZipFile(root/'records.zip')as archive:return archive.read(name)
def run(root,native=None):
 for name,digest in {'source.json': '8eff9f683b07ad4103f5f5d38b34dc108288b7937c83534a849b4a93d7376263', 'source-app.json': '3f975fa3fba86a1278534723e92c78f6cb7acfb46a000bda42790dd4b0971ce8'}.items():assert hashlib.sha256(read(root,name)).hexdigest()==digest,("source recipe bytes",name)
 s=json.loads(read(root,'source.json'));n=json.loads(Path(native).read_bytes()if native else read(root,'native.json'));a=json.loads(read(root,'source-app.json'))
 assert s['source_pin']==a['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [x['name']for x in s['cases']]==[x['name']for x in n['cases']]==NAMES
 results=[]
 for x,y in zip(s['cases'],n['cases']):
  try:check_mesh(x['after'],y['after']);passed=True;error=None
  except (AssertionError,KeyError)as e:passed=False;error=str(e)
  assert passed==(x['name']!='oblique-cap'),(x['name'],error)
  if not passed:assert error=="('edges', 38, 36)" and len(y['after']['vertices'])==26 and len(y['after']['edges'])==36 and len(y['after']['faces'])==14,'changed retained failure'
  results.append(dict(name=x['name'],passed=passed,error=error))
 assert [x['name']for x in a['apps']]==[x['name']for x in n['apps']]==['endpoint-free-wire','open-neighbor','nested-translated']
 app=[]
 for x,y in zip(a['apps'],n['apps']):
  expected=PHASES+(['finish-child','finish-outer','finish-undo','finish-redo']if x['name']=='nested-translated'else[])
  assert set(x['states'])==set(y['states'])==set(expected)
  for phase in expected:
   p,q=x['states'][phase],y['states'][phase]
   context_finish=x['name']=='nested-translated' and phase in ['finish-child','finish-outer','finish-redo']
   allowed=[s['cases'][0]['before']['vertices'][i]for i in s['cases'][0]['before']['edges'][s['cases'][0]['selected'][0]]['ends']]if context_finish else False
   assert len(q['mesh']['vertices'])==len(p['mesh']['vertices'])+(2 if context_finish else 0),'unexpected raw vertex inventory'
   curve_exact=True
   try:check_mesh(p['mesh'],q['mesh'],False,allowed)
   except AssertionError as error:
    assert x['name']=='nested-translated' and phase in ['finish-child','finish-outer','finish-redo'] and str(error)=='curve partition',(x['name'],phase,str(error));curve_exact=False
    check_mesh(p['mesh'],q['mesh'],False,allowed,False)
   assert curve_exact==(not(x['name']=='nested-translated' and phase in ['finish-child','finish-outer','finish-redo']))
   for key in ['undo','redo','selection','preview_faces','preview_lines']:assert p[key]==q[key],(x['name'],phase,key,p[key],q[key])
   app.append(dict(name=x['name'],phase=phase,literal_curve_partition=curve_exact,raw_vertices=[len(p['mesh']['vertices']),len(q['mesh']['vertices'])],literal_vertex_inventory=len(p['mesh']['vertices'])==len(q['mesh']['vertices']),interior_counts=[sum(f['interior']for f in p['mesh']['faces']),sum(f['interior']for f in q['mesh']['faces'])]))
 # Actual checker negatives, mutate admitted native corpus rather than synthetic pass counters.
 baseline=n['cases'][0]['after'];source=s['cases'][0]['after'];neg=[]
 for name in ['lost-wire','wrong-interior','wrong-paint','lost-identity','reverse-face','lost-face','curve-merge']:
  m=copy.deepcopy(baseline)
  if name=='lost-wire':m['edges']=[e for e in m['edges']if e['curve']!=41]
  elif name=='wrong-interior':m['faces'][0]['interior']=not m['faces'][0]['interior']
  elif name=='wrong-paint':m['faces'][-1]['attrs']={'vendor':'wrong'}
  elif name=='lost-identity':m['edges'][0]['origin']=999
  elif name=='reverse-face':m['faces'][-1]['loop'].reverse()
  elif name=='lost-face':m['faces'].pop()
  else:
   for e in m['edges']:
    if e['curve']is not None:e['curve']=41
  try:check_mesh(source,m)
  except (AssertionError,KeyError):neg.append(name)
  else:raise AssertionError(('negative accepted',name))
 return dict(scope='Five source-matched topology/metadata preservation recipes, one retained oblique topology failure; actual handler/application states, no rendered pointer claim. Nested source context cloning drops unused vertices, native retains them; literal vertex inventory and Finish curve partition divergence retained per phase, only unreferenced extra vertices permitted. New global source curve numbers normalized by partition only; old curve IDs literal. Geometry source Qt-f32/native f64 existing 1e-6 boundary.',model=results,application_states=app,negatives=neg)
if __name__=='__main__':
 result=run(Path(sys.argv[1]),sys.argv[3]if len(sys.argv)>3 else None);print(json.dumps(result,indent=2));
 if len(sys.argv)>2:Path(sys.argv[2]).write_text(json.dumps(result,indent=2)+'\n')
