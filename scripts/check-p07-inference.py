#!/usr/bin/env python3
"""Strict frozen P07 inventory, source decisions and numeric/App admission."""
import copy,json,math,struct,sys
from pathlib import Path
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
VIEWS=['xy','xz','yz','oblique'];LABELS=['cross','inside14','exact14','outside14','skew','parallel','occluded','guide-model','point-inside9','point-exact9','point-outside9','point-priority'];APPS=['guide-line-'+x for x in VIEWS]+['rectangle-locale','circle-locale-count','arc-locale-radius','invalid-locale-refusal'];PHASES=['before','press','hover','commit-or-refuse','cancel','undo','redo']
def same(a,b):
 assert type(a)is type(b),(a,b)
 if isinstance(a,dict):assert a.keys()==b.keys();[same(a[k],b[k])for k in a]
 elif isinstance(a,list):assert len(a)==len(b);[same(x,y)for x,y in zip(a,b)]
 elif isinstance(a,float):assert math.isfinite(a)and math.isfinite(b)and a==b,(a,b)
 else:assert a==b,(a,b)
def validate(s,a,n):
 assert s['source_pin']==a['source_pin']==PIN
 assert [r['name']for r in s['cases']]==[v+'-'+label for v in VIEWS for label in LABELS]
 assert [r['name']for r in n['cases']]==[r['name']for r in s['cases']]
 assert len(s['numeric'])==len(n['numeric'])==286 and s['unicode_version']=='15.1.0'and len(s['decimal_zeroes'])==68
 assert [r['recipe']['name']for r in a['cases']]==[r['name']for r in n['app']]==APPS
 failures=[]
 for original,native in zip(s['cases'],n['cases']):
  try:
   assert native['kind']==original['source']['kind'];assert len(native['point'])==3
   assert all(type(x)is float and math.isfinite(x)for x in native['point'])
   assert max(abs(x-y)for x,y in zip(native['point'],original['source']['point']))<1e-6
   assert len(native['segments'])==len(original['segments'])
   for x,y in zip(native['segments'],original['segments']):
    assert all(struct.pack('!f',i)==struct.pack('!f',j)for p,q in zip(x,y)for i,j in zip(p,q)),('clip endpoint bits',x,y)
  except AssertionError as e:failures.append({'case':original['name'],'source':original['source'],'native':native,'error':str(e)})
 for original,native in zip(s['numeric'],n['numeric']):
  try:same(original,native)
  except AssertionError as e:failures.append({'numeric':original['text'],'source':original,'native':native,'error':str(e)})
 for original,native in zip(a['cases'],n['app']):
  try:
   assert [x['phase']for x in original['states']]==[x['phase']for x in native['states']]==PHASES
   assert native['accepted']is(original['recipe']['name']!='invalid-locale-refusal')
   assert all(x is True for x in original['characters_accepted'])
   for x,y in zip(original['states'],native['states']):same(x,y)
  except AssertionError as e:failures.append({'app':native['name'],'error':str(e),'source':original,'native':native})
 assert n['ascii_borrowed']is True and n['native_f64_preserved']is True and n['double_guide_matrix_preserved']is True
 try:
  e=n['egui'];same(e['anchor'],e['expected_anchor']);assert e['buffered']=='٠,٥';same(e['after'],e['redo']);assert e['after']['vertices']==[[1.0,1.0,0.0],[1.5,1.0,0.0]];assert len(e['after']['edges'])==1 and e['after']['history']==[1,0];assert e['undo']['edges']==[] and e['undo']['history']==[0,1]
 except AssertionError as error:failures.append({'egui':'source guide acquisition + Arabic VCB text/Enter/history','error':str(error),'native':n['egui']})
 return failures
s=json.loads(Path(sys.argv[1]).read_text());a=json.loads(Path(sys.argv[2]).read_text());n=json.loads(Path(sys.argv[3]).read_text());failures=validate(s,a,n);controls=[]
if not failures:
 for label,mutate in [('missing-guide',lambda x:x['cases'].pop()),('wrong-snap-kind',lambda x:x['cases'][0].update(kind='none')),('wrong-snap-point',lambda x:x['cases'][0]['point'].__setitem__(0,10.0)),('wrong-numeric-type',lambda x:x['numeric'][0].update(value=[True])),('inert-vcb',lambda x:x['app'][0].update(accepted=False)),('lost-history',lambda x:x['app'][0]['states'][-1]['state'].update(history=[0,0])),('inert-egui',lambda x:x['egui'].update(anchor=None)),('wrong-typed-stroke',lambda x:x['egui']['after']['vertices'][1].__setitem__(0,1.6))]:
  wrong=copy.deepcopy(n);mutate(wrong)
  try:rejected=bool(validate(s,a,wrong))
  except AssertionError:rejected=True
  assert rejected,label;controls.append(label)
report={'guide_cases':48,'numeric_rows':286,'app_cases':8,'app_phases':56,'failures':failures,'negative_controls':controls,'scope':'Source guide clipping endpoint f32 bits, 3D decisions and unchanged established1e-6 snap boundary; exact numeric values/types and five-decimal authored App model/history. No rendered/OS gestures or entire P07 promotion.'}
Path(sys.argv[4]).write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n');print(json.dumps({'failures':len(failures),'guide_cases':48,'numeric':286,'app':8,'negative_controls':len(controls)}));assert not failures
