import json,copy,sys
from pathlib import Path
p=Path(sys.argv[1]);m=json.loads((p/'memory.json').read_text())
def validate(x):
 assert x['base_vertices']==10000 and x['history_depth']==96
 assert [v['depth']for v in x['memory_points']]==[32,64,96]
 assert all(v['retained_baseline_entries']==0 for v in x['memory_points'])
 assert x['retained_baseline_entries']==0 and 0<x['history_growth_bytes']<2*1024*1024
 assert x['replace_released_bytes']>1000000 and x['undo_redo_cycles']==96
 assert not x['production_writers_routed'] and not x['system_allocator_oom_proof']
validate(m);bad=[]
for name in ['base-per-command','quadratic-memory-growth','history-not-reclaimed']:
 x=copy.deepcopy(m)
 if name=='base-per-command':x['retained_baseline_entries']=10000
 elif name=='quadratic-memory-growth':x['history_growth_bytes']=40000000
 else:x['replace_released_bytes']=0
 try:validate(x)
 except AssertionError:bad.append(name)
 else:raise AssertionError('mutation accepted '+name)
assert json.loads((p/'native.json').read_text())==json.loads((p/'original.json').read_text())
print(json.dumps(dict(source_history_states=12,history_depth=96,observed_growth_bytes=m['history_growth_bytes'],negative_rejections=bad,allocation_scope='live Rust allocator request bytes, not RSS',production=False),indent=2))
