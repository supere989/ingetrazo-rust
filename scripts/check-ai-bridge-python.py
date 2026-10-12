#!/usr/bin/env python3
"""Strict actual socket receipts and App states; raw interpreter differences remain failures."""
import argparse,copy,json,math
from pathlib import Path
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
ROSTER=[('fragmented-and-ordered-requests',2),('stdout-stderr-eval-persistent-scope',4),('geometry-history',3),('noop-preserves-redo',3),('mutation-error-rollback',4),('syntax-type-dispatch-errors',4)]
CONTROLS={'actual_socket','actual_ui_poll','caught_overflow_atomic','disabled_refused','modal_refused','worker_joined'}
def exact(a,b,path='$'):
 assert type(a)is type(b),path+': type'
 if isinstance(a,dict):
  assert set(a)==set(b),path+': keys'
  for k in a:exact(a[k],b[k],path+'.'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),path+': length'
  for i,(x,y)in enumerate(zip(a,b)):exact(x,y,path+'['+str(i)+']')
 elif isinstance(a,float):assert math.isfinite(a)and math.isfinite(b)and a==b and (a!=0 or math.copysign(1,a)==math.copysign(1,b)),path+': number'
 else:assert a==b,path+': value'
def scene(v):
 assert type(v)is dict and set(v)=={'faces','undo','redo','groups','layers'}
 for k in ['undo','redo','groups','layers']:assert type(v[k])is int and v[k]>=0
 assert type(v['faces'])is list
 def point(p):assert type(p)is list and len(p)==3 and all(type(x)is float and math.isfinite(x)for x in p)
 for f in v['faces']:
  assert type(f)is dict and set(f)=={'outer','holes','attrs'}and type(f['attrs'])is dict
  assert type(f['outer'])is list and len(f['outer'])>=3 and type(f['holes'])is list
  for ring in [f['outer']]+f['holes']:
   assert type(ring)is list and len(ring)>=3
   for p in ring:point(p)
def inventory(v):
 assert v['source_pin']==PIN
 assert type(v['cases'])is list and [(c['name'],len(c['requests']))for c in v['cases']]==ROSTER
 assert type(v['states'])is list and [(s['case'],s['index'])for s in v['states']]==[(n,i)for n,c in ROSTER for i in range(c)]
 for s in v['states']:
  assert type(s)is dict and set(s)=={'case','index','reply','scene'}and type(s['index'])is int
  scene(s['scene']);r=s['reply'];assert type(r)is dict and type(r.get('ok'))is bool and 'id'in r
  if r['ok']:
   assert set(r)=={'id','ok','result'}and type(r['result'])is dict
   result=r['result']
   if set(result)=={'ok'}:assert type(result['ok'])is bool
   else:
    assert set(result)=={'stdout','stderr','error','changed'}
    assert type(result['stdout'])is str and type(result['stderr'])is str and type(result['changed'])is bool
    assert result['error']is None or type(result['error'])is str
  else:assert set(r)=={'id','ok','error'}and type(r['error'])is str
 assert any(s['scene']['faces']for s in v['states']),'joint empty geometry'
def compare(a,b):
 inventory(a);inventory(b);assert a['callback_errors']==[]
 assert type(b['controls'])is dict and set(b['controls'])==CONTROLS and all(type(v)is bool and v for v in b['controls'].values())
 assert set(b['control_replies'])=={'overflow','disabled','modal'}
 for r in b['control_replies'].values():assert type(r)is dict and r.get('ok')is False and type(r.get('error'))is str
 exact(a['cases'],b['cases'],'cases');exact(a['states'],b['states'],'states')
def main():
 ap=argparse.ArgumentParser();ap.add_argument('source',type=Path);ap.add_argument('native',type=Path);ap.add_argument('comparison',type=Path);args=ap.parse_args()
 result={'passed':False,'scope':'Exact six actual socket cohorts/20 raw replies and typed App states; no traceback rewriting, numeric tolerance or unsupported Python substitution.','negative_controls_accepted':False}
 try:
  a=json.loads((args.source/'original-ai-bridge-python.json').read_text());b=json.loads((args.native/'native-ai-bridge-python.json').read_text())
  # Separate diagnostic counts never replace full strict acceptance.
  count=0
  for x,y in zip(a['states'],b['states']):
   try:exact(x['scene'],y['scene'])
   except (AssertionError,KeyError,TypeError):pass
   else:count+=1
  result['exact_app_states']=count
  compare(a,b);probes=[]
  mutations=[('empty-roster',lambda n:n.update(cases=[])),('truncated-states',lambda n:n['states'].pop()),('reordered-states',lambda n:n['states'].reverse()),('wrong-pin',lambda n:n.update(source_pin='wrong')),('false-control',lambda n:n['controls'].update(actual_socket=False)),('numeric-control',lambda n:n['controls'].update(actual_socket=1)),('joint-empty-geometry',lambda n:[s['scene'].update(faces=[])for s in n['states']]),('changed-stdout',lambda n:n['states'][0]['reply']['result'].update(stdout='corrupt')),('changed-error',lambda n:n['states'][13]['reply']['result'].update(error='corrupt')),('bool-history',lambda n:n['states'][0]['scene'].update(undo=False)),('changed-history',lambda n:n['states'][6]['scene'].update(undo=99)),('successful-overflow',lambda n:n['control_replies']['overflow'].update(ok=True))]
  for name,mutate in mutations:
   n=copy.deepcopy(b);mutate(n)
   try:compare(a,n)
   except (AssertionError,KeyError,TypeError,ValueError):probes.append(name)
   else:raise AssertionError('negative escaped: '+name)
  result.update(passed=True,cases=6,replies=20,negative_controls=probes,negative_controls_accepted=True)
 except (AssertionError,KeyError,TypeError,ValueError,OSError)as e:result['error']=repr(e)
 args.comparison.parent.mkdir(parents=True,exist_ok=True);args.comparison.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));return 0 if result['passed']else 1
if __name__=='__main__':raise SystemExit(main())
