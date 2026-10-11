"""Strict recursive graph inventory plus operative callback outcomes; full parity remains false."""
import sys,json,copy,re
from pathlib import Path
def document(path):
 text=Path(path).read_text()
 if text.lstrip().startswith('{'):return json.loads(text)
 return json.loads(next(line for line in reversed(text.splitlines()) if line.startswith('{')))
def compare(expected,actual,path='$'):
 if type(expected)is not type(actual):raise AssertionError((path,'type',type(actual).__name__))
 if isinstance(expected,dict):
  assert expected.keys()==actual.keys(),(path,'keys')
  for key in expected:compare(expected[key],actual[key],path+'.'+key)
 elif isinstance(expected,list):
  assert len(expected)==len(actual),(path,'length')
  for i,(a,b) in enumerate(zip(expected,actual)):compare(a,b,f'{path}[{i}]')
 else:assert expected==actual,(path,expected,actual)
def validate_graph(expected,actual):
 assert len(expected['states'])==13 and len({s['name']for s in expected['states']})==13,'mandatory original inventory'
 assert expected['production_enabled']is False
 compare(expected,actual)
def callbacks(path):
 line=next(line for line in reversed(Path(path).read_text().splitlines()) if line.startswith('CALLBACK_COUNTS'))
 values=line.split();assert len(values)==3
 return [int(values[1]),int(values[2])]
original,native,eqoriginal,eqnative,cbsource,cbnative,cbold=sys.argv[1:8]
e= document(original);a=document(native);validate_graph(e,a)
mutations=[]
for name,edit in [
 ('missing-shared-radial',lambda x:x['states'][0]['radial'][0].pop()),
 ('wrong-incidence',lambda x:x['states'][0]['incidence'][0].append(4)),
 ('missing-public-edge',lambda x:x['states'][0]['edges'].pop()),
 ('wrong-wrapper-identity',lambda x:x['states'][2].__setitem__('held_radial_original',True)),
 ('wrong-endpoint-restoration',lambda x:x['states'][10]['endpoints'][0].__setitem__(0,0)),
 ('lost-history',lambda x:x['states'][2].__setitem__('redo',0)),
 ('production-claim',lambda x:x.__setitem__('production_enabled',True)),
 ('empty-inventory',lambda x:x.__setitem__('states',[])),
 ('bool-as-number',lambda x:x['states'][0]['persistent_lists'].__setitem__(0,1))]:
 bad=copy.deepcopy(a);edit(bad)
 try:validate_graph(e,bad)
 except AssertionError:mutations.append(name)
 else:raise AssertionError(('mutation accepted',name))
expected=document(eqoriginal);actual=document(eqnative)
assert [x['mode']for x in expected['cases']]==['false','true','raise','mutating-same','mutating-field','geometry-short-circuit']
assert [x['mode']for x in actual['cases']]==[x['mode']for x in expected['cases']]
matched=[];boundaries=[]
for s,n in zip(expected['cases'],actual['cases']):
 if s['mode']in ['false','true','mutating-same','geometry-short-circuit']:compare(s,n);matched.append(s['mode'])
 else:
  assert s['state']['calls']==n['state']['calls']==1
  assert s['state']['undo']==2 and s['state']['redo']==0 and s['state']['payload_is_new']is True
  assert n['state']['undo']==1 and n['state']['redo']==1 and n['state']['payload_is_old']is True
  assert 'Err'in n['result']
  if s['mode']=='raise':assert s['result']=={'Err':'ValueError: eq-failure'}
  else:assert s['result']=={'Ok':True}and s['state']['hidden']is True and n['state']['hidden']is False
  boundaries.append({'mode':s['mode'],'source':s,'native':n})
compare([1,1],callbacks(cbsource));compare([1,1],callbacks(cbnative))
try:compare([1,1],callbacks(cbold))
except AssertionError:old_counts_rejected=True
else:raise AssertionError('literal old counts accepted')
print(json.dumps({'complete_graph_states':13,'recursive_mutations_rejected':mutations,'callback_positive_cases_exact':matched,'hash_callback_control':True,'literal_old_callback_count_rejected':old_counts_rejected,'required_open_boundaries':boundaries,'full_callback_parity':False,'production_enabled':False},indent=2))
