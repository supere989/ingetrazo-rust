import json,copy,sys,runpy
from pathlib import Path
p=Path(sys.argv[1]);runpy.run_path(str(Path(__file__).with_name('check-ai-private-owner.py')),run_name='__main__')
data=json.loads((p/'memory.json').read_text())
def validate(m):
 b=m['owner_budget'];h=m['retained_history_payload_bytes'];assert b['history']==sum(h)>0
 assert b['staging']==0 and b['preparation']==0 and b['peak']>=b['history']
 assert b['vm_memory_included'] is False and b['application_mesh_included'] is False
 assert all(x['history_payload_bytes']==[h[0]*x['depth']//96,0] for x in m['memory_points'])
validate(data);rejected=[]
for name in ['history-charge-lost','staging-leak','false-vm-coverage']:
 x=copy.deepcopy(data)
 if name=='history-charge-lost':x['owner_budget']['history']=0
 elif name=='staging-leak':x['owner_budget']['staging']=1
 else:x['owner_budget']['vm_memory_included']=True
 try:validate(x)
 except (AssertionError,KeyError):rejected.append(name)
 else:raise AssertionError('Accepted '+name)
print(json.dumps({'shared_budget_negative_rejections':rejected,'logical_budget_not_allocator_cap':True,'production':False},indent=2))
