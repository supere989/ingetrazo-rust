#!/usr/bin/env python3
"""Strict missing P07 producer/camera/guide input and App contracts."""
import copy,json,math,sys
from pathlib import Path
CAMERAS=['perspective','close','two-point','steep-fallback','parallel']
LABELS=['cross','point-inside9','point-exact9','point-outside9','line-inside14','line-exact14','line-outside14']
def validate(s,n):
 assert s['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 names=[c+'-'+k for c in CAMERAS for k in LABELS]
 assert [r['name']for r in s['cases']]==[r['name']for r in n['cases']]==names
 assert [r['name']for r in n['egui']]==[c+'-cross'for c in CAMERAS]
 assert n['native_double_preserved']is True
 errors=[]
 def check(label,fn):
  try:fn()
  except AssertionError as e:errors.append({'field':label,'error':str(e)})
 def same(a,b):
  assert type(a)is type(b),(a,b)
  if isinstance(a,dict):
   assert a.keys()==b.keys(),(a.keys(),b.keys())
   for k in a:same(a[k],b[k])
  elif isinstance(a,list):
   assert len(a)==len(b)
   for x,y in zip(a,b):same(x,y)
  else:assert a==b,(a,b)
 def snap(a,b):
  assert a['kind']==b['kind'],(a,b)
  assert len(b['point'])==3 and all(type(x)is float and math.isfinite(x)for x in b['point'])
  assert max(abs(x-y)for x,y in zip(a['point'],b['point']))<1e-6,(a,b)
 for a,b in zip(s['cases'],n['cases']):
  for field in ['authored','matrix','segments','states']:check(a['name']+'.'+field,lambda a=a,b=b,field=field:same(a[field],b[field]))
  check(a['name']+'.snap',lambda a=a,b=b:snap(a['source'],b['source']))
  check(a['name']+'.production_context',lambda a=a,b=b:snap(a['production_context'],b['production_context']))
 for e in n['egui']:
  source=next(r for r in s['cases']if r['name']==e['name']);expected={r['phase']:r['state']for r in source['states']};native=e['result']
  check(e['name']+'.egui.anchor',lambda:same([round(x,5)for x in source['production_context']['point']],native['anchor']))
  check(e['name']+'.egui.accepted',lambda:same(True,native['accepted']))
  for ours,theirs in [('after','line'),('undo','undo'),('redo','redo')]:check(e['name']+'.egui.'+ours,lambda ours=ours,theirs=theirs:same(expected[theirs],native[ours]))
 return errors
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text());errors=validate(s,n);negative=[]
if not errors:
 for name,mutate in [('missing-camera',lambda n:n['cases'].pop()),('wrong-matrix',lambda n:n['cases'][0]['matrix'].__setitem__(0,1.)),('unclipped-guide',lambda n:n['cases'][0]['segments'][0][0].__setitem__(0,10000.)),('wrong-snap',lambda n:n['cases'][0]['source'].update(kind='none')),('wrong-guide',lambda n:n['cases'][0]['authored'][0]['direction'].__setitem__(0,1.)),('lost-history',lambda n:n['cases'][0]['states'][-1]['state'].update(history=[0,0])),('inert-egui',lambda n:n['egui'][0]['result'].update(anchor=None))]:
  v=copy.deepcopy(n);mutate(v)
  try:rejected=bool(validate(s,v))
  except AssertionError:rejected=True
  assert rejected,name;negative.append(name)
report={'cases':35,'actual_camera_modes':5,'app_states':55,'actual_egui_sessions':5,'failures':errors,'negative_controls':negative,'scope':'Exact guide producer data, camera matrix coefficients and clipped span float values; unchanged 1e-6 world snap tolerance; actual App/egui authoring and history at five-decimal mesh precision. No whole-frame or complete Protractor claim.'}
Path(sys.argv[3]).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'failures':len(errors),'first':errors[:6],'negatives':len(negative)}));sys.exit(bool(errors))
