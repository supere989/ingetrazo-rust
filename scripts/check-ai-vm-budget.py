import json,copy,sys
from pathlib import Path
p=Path(sys.argv[1]);x=json.loads((p/'native.json').read_text());names=['caught-baseexception','nested-frames','generator','trace-disabled','script-reset-unavailable','native-callback-reentry','dummy-thread-callback','destructor-unwind','finally-block']
def validate(d):
 assert d['fuel']==2000 and d['production']==False and d['wall_time_bound']==False
 assert [c['name']for c in d['cases']]==names
 for c in d['cases']:
  assert c['poisoned'] and c['staged_edges_discarded']==1 and c['live_edges']==0
  assert c['recovered_value']=='63' and c['finally_marker']=='False'
validate(x);bad=[]
for name in ['caught-poison','published-after-poison','recovery-not-checked','missing-finalizer']:
 y=copy.deepcopy(x)
 if name=='caught-poison':y['cases'][0]['poisoned']=False
 elif name=='published-after-poison':y['cases'][-2]['live_edges']=1
 elif name=='recovery-not-checked':y['cases'][1]['recovered_value']='0'
 else:y['cases'].pop(-2)
 try:validate(y)
 except AssertionError:bad.append(name)
 else:raise AssertionError('mutation accepted '+name)
old=json.loads((p/'nohook-negative.json').read_text());assert old['exit_code']==124 and not old['valid_native_result']
f=json.loads((p/'dependency-boundary.json').read_text());assert 'threading'not in f['features'] and 'jit'not in f['features']
print(json.dumps(dict(cases=9,poison_controls_pass=True,literal_nohook_timeout_rejected=True,negative_rejections=bad,wall_time_bound=False,production=False),indent=2))
