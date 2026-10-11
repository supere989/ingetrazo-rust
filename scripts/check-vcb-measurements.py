#!/usr/bin/env python3
import copy,json,struct,sys
from pathlib import Path
p=Path(sys.argv[1]);s=json.loads((p/'original-measurements.json').read_text());n=json.loads((p/'native-measurements.json').read_text())
assert s['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
assert len(s['rows'])==1168 and set(r['unit']for r in s['rows'])=={'m','cm','mm','in','ft','ft-in','in-frac','ft-in-frac'}
def exact(a,b,path=''):
 assert type(a)is type(b),(path,'type',a,b)
 if isinstance(a,dict):
  assert a.keys()==b.keys(),path
  for k in a:exact(a[k],b[k],path+'/'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),path
  for i,(x,y)in enumerate(zip(a,b)):exact(x,y,path+'/'+str(i))
 elif isinstance(a,float):assert struct.pack('!d',a)==struct.pack('!d',b),(path,a,b)
 else:assert a==b,(path,a,b)
exact(s['rows'],n['rows'])
negative=[]
for label,predicate,value in [
 ('bare-mm-is-not-metres',lambda r:r['unit']=='mm'and r['text']=='2'and not r['unitless'],[2.0]),
 ('composite-bare-is-inch',lambda r:r['unit']=='ft-in'and r['text']=='2'and not r['unitless'],[2.0]),
 ('unitless-factor-stays-unitless',lambda r:r['unit']=='mm'and r['text']=='2'and r['unitless'],[0.002]),
 ('comma-is-decimal',lambda r:r['unit']=='m'and r['text']=='200,100'and not r['comma_lists'],[200.0,100.0]),
 ('rectangle-comma-is-list',lambda r:r['unit']=='m'and r['text']=='200,100'and r['comma_lists'],[200.1]),
 ('mixed-inch-conversion',lambda r:r['unit']=='m'and r['text']=='1 1/2"'and not r['unitless'],[1.5]),
 ('reject-overflow-token',lambda r:r['unit']=='m'and r['text']=='1000000001'and not r['unitless'],[1000000001.0]),
 ('exact-numeric-type',lambda r:r['unit']=='m'and r['text']=='2'and not r['unitless'],[2]),
]:
 bad=copy.deepcopy(n['rows']);next(r for r in bad if predicate(r))['value']=value
 try:exact(s['rows'],bad)
 except AssertionError:negative.append(label)
 else:raise AssertionError('accepted negative '+label)
workflows=json.loads((p/'app-measurement-workflows.json').read_text())['workflows']
expected=['line','line-delta','rectangle','move','array','pushpull','offset','fillet','tape','arc','circle','circle-count','rotate','scale','scale-absolute']
assert [r['case']for r in workflows]==expected
for row in workflows:
 state=row['states'];assert state['commit']!=state['before'],row['case'];exact(state['commit'],state['redo'])
 assert state['commit']['undo']==1 and state['undo']['redo']==1,row['case']
print(json.dumps(dict(source_rows=len(s['rows']),typed_and_float_bits_exact=True,negative_controls=negative,actual_app_unit_routing_cases=len(workflows),phases_per_app_case=4,scope='Source numeric grammar plus actual App physical-control equivalence; broader whole-tool source geometry and keyboard dispatch remain separately tracked.')))
