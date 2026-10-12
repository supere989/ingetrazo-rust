#!/usr/bin/env python3
"""Strict frozen import states, bytes and actual original-reader pairs."""
import copy,json,math,sys
from pathlib import Path
D='stl'
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
 a=json.loads((s/'original-stl.json').read_text());b=json.loads((n/'native-stl.json').read_text())
 assert a['source_pin']==b['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert a['callback_errors']==[]
 names=['default-cube','none-cube','tilted-principal','tilted-all','flat-large-editable','large-root','large-nested','small-nested','unit-cancel','chooser-cancel','parse-failure']
 assert [c['name']for c in a['cases']]==names
 inventory=[(name,phase)for i,name in enumerate(names)for phase in (['before','import','undo','redo']if i<8 else['before','import'])]
 assert [(v['case'],v['phase'])for v in a['states']]==inventory
 def states(v):
  rows=copy.deepcopy(v['states'])
  for row in rows:row.pop('camera')
  return rows
 expected=states(a);exact(expected,states(b))
 exact([{k:c[k]for k in ['name','saved','phases']}for c in a['cases']],b['cases'])
 controls=['solid-binary','trailing','ignored-normal','ASCII-BOM-CR','degenerate','nonfinite-coordinate','invalid-scale','invalid-mode']
 assert [c['name']for c in a['decoder_controls']]==controls
 def decodes(v):
  rows=copy.deepcopy(v['decoder_controls'])
  for row in rows:
   row.pop('source_diagnostic',None);row.pop('native_diagnostic',None)
  return rows
 exact(decodes(a),decodes(b))
 readers=json.loads((n/'reopens-stl.json').read_text());assert [v['case']for v in readers]==names[:8]
 for row in readers:exact(row['original'],row['native'])
 for k in ['actual_ingress','atomic_failure','parse_once','worker_mesh_only','camera_unchanged']:assert b['guards'].get(k)is True,('guard',k)
 probes=[]
 for key in ['undo','redo']:
  c=copy.deepcopy(b);c['states'][0][key]+=1;probes.append(c)
 c=copy.deepcopy(b);c['states'].pop();probes.append(c)
 c=copy.deepcopy(b);c['states'][0]['scene']={'corruption':True};probes.append(c)
 c=copy.deepcopy(b);c['states'][0]['preferences']['import/stl_unit']='ft';probes.append(c)
 for c in probes:
  try:exact(expected,states(c))
  except AssertionError:pass
  else:raise AssertionError('state negative accepted')
 for index in [0,7]:
  c=copy.deepcopy(b);c['decoder_controls'][index]['outcome']='corrupt'
  try:exact(decodes(a),decodes(c))
  except AssertionError:pass
  else:raise AssertionError('decoder negative accepted')
 r.update(ok=True,cases=11,states=38,decoder_controls=8,readers=8,negative_controls=7,diagnostic_scope='Camera values and parser error text diagnostic only; native unchanged-camera guard required. Full model/history/preferences and accepted private meshes strict.')
except Exception as e:r['error']=repr(e)
report.parent.mkdir(parents=True,exist_ok=True);report.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r));sys.exit(0 if r['ok']else 1)
