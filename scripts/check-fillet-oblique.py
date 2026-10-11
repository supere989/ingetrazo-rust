#!/usr/bin/env python3
"""Strict repaired-source oblique topology and App history; no renderer acceptance."""
import json,sys,hashlib,zipfile,copy,collections
from pathlib import Path
import runpy
check_mesh=runpy.run_path(str(Path(__file__).with_name('check-fillet-scoped.py')),run_name='fillet_proof_lib')['check_mesh']
NAMES=['oblique-cap','turned-squashed'];PHASES=['activate','sizing','commit','undo','redo']
SOURCE_SHA='cd04f87be62db6b0bd066c1402bcd9e007f1531c62d5fed65099ac4b965084d4'
def read(root,name):
 if(root/name).is_file():return(root/name).read_bytes()
 with zipfile.ZipFile(root/'records.zip')as archive:return archive.read(name)
def closed(mesh):
 incidence=collections.Counter();directed=collections.Counter()
 for f in mesh['faces']:
  for ring in [f['loop']]+f['holes']:
   for a,b in zip(ring,ring[1:]+ring[:1]):incidence[tuple(sorted((a,b)))]+=1;directed[a,b]+=1
 assert incidence and all(n==2 for n in incidence.values()),'open or nonmanifold boundary'
 assert all(directed[a,b]==directed[b,a]for a,b in directed),'inconsistent winding'
 assert {tuple(sorted(e['ends']))for e in mesh['edges']}==set(incidence),'missing/extra surface edge'
def model(source,native):
 check_mesh(source,native);closed(source);closed(native)
def run(root,native=None):
 raw=read(root,'source-fixed.json');assert hashlib.sha256(raw).hexdigest()==SOURCE_SHA,'required source recipe bytes'
 source=json.loads(raw);out=json.loads(Path(native).read_bytes()if native else read(root,'native.json'))
 assert source['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [r['name']for r in source['cases']]==[r['name']for r in out['cases']]==NAMES
 old=json.loads(read(root,'source-invalid-registry.json'))
 report=[]
 for s,n in zip(source['cases'],out['cases']):
  previous=next(row for row in old['cases']if row['name']==s['name']);assert previous['before']==s['model']['before'],'authoring positions/metadata changed'
  model(s['model']['after'],n['model']['after'])
  expected=(26,36,14)if s['name']=='oblique-cap'else(128,300,182)
  assert tuple(len(n['model']['after'][k])for k in ['vertices','edges','faces'])==expected
  assert list(s['states'])==PHASES and set(n['states'])==set(PHASES)
  for phase in PHASES:
   check_mesh(s['states'][phase]['mesh'],n['states'][phase]['mesh'],False);closed(n['states'][phase]['mesh'])
   for key in ['undo','redo','selection','preview_faces','preview_lines']:assert s['states'][phase][key]==n['states'][phase][key],(s['name'],phase,key)
  if s['name']=='oblique-cap':
   mesh=n['model']['after'];caps=[f for f in mesh['faces']if len(f['loop'])>4 and all(abs(mesh['vertices'][i][0]+.5*mesh['vertices'][i][1]-2.5)<1e-6 for i in f['loop'])];assert len(caps)==1,'original oblique-cap plane invariant'
  else:check_mesh(previous['after'],s['model']['after'])
  report.append(dict(name=s['name'],counts=expected,phases=PHASES,passed=True))
 bad=old['cases'][3]['after'];failure=None
 try:closed(bad)
 except AssertionError as error:failure=str(error)
 assert failure=='open or nonmanifold boundary'
 negatives=[];baseline=out['cases'][0]['model']['after'];wanted=source['cases'][0]['model']['after']
 for name in ['invalid-old-registry','removed-edge','reversed-face','wrong-paint','wrong-interior','lost-old-identity','hidden-edge','old-vertex-identity']:
  m=copy.deepcopy(baseline)
  if name=='invalid-old-registry':m=copy.deepcopy(bad)
  elif name=='removed-edge':m['edges'].pop()
  elif name=='reversed-face':m['faces'][-1]['loop'].reverse()
  elif name=='wrong-paint':m['faces'][-1]['attrs']={'vendor':'wrong'}
  elif name=='wrong-interior':m['faces'][0]['interior']=True
  elif name=='lost-old-identity':m['edges'][0]['origin']=None
  elif name=='hidden-edge':m['edges'][0]['hidden']=True
  else:m['vertex_origins'][0]=None
  try:model(wanted,m)
  except(AssertionError,KeyError):negatives.append(name)
  else:raise AssertionError(('negative accepted',name))
 return dict(scope='Two affected source constructors corrected with actual Mesh.place_vertex. Full oriented topology/metadata/old identity and ten real App handler/history phases; no rendered pointer acceptance. Existing1e-6 Qt/f64 boundary, new curve IDs compare partitions.',cases=report,invalid_original_registry_fixture=dict(counts=[27,38,14],closed=False,boundary_edges=4,reason=failure),negatives=negatives)
if __name__=='__main__':
 result=run(Path(sys.argv[1]),sys.argv[3]if len(sys.argv)>3 else None);print(json.dumps(result,indent=2))
 if len(sys.argv)>2:Path(sys.argv[2]).write_text(json.dumps(result,indent=2)+'\n')
