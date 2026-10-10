#!/usr/bin/env python3
import json,copy,sys
from pathlib import Path
NAMES=['append-and-weld','merged-noop','retain-existing-edge','partial-error','expression-noop','same-edge-object','existing-reference-after-error','detached-pending-reference']
def validate(a,b):
 assert [x['name']for x in a['cases']]==NAMES
 assert [x['name']for x in b['cases']]==NAMES
 for x,y in zip(a['cases'],b['cases']):
  for key in ['name','changed','failed','stdout','vertices','edges']:assert x[key]==y[key],(x['name'],key)
 assert b['cases'][3]['staged_edges']==1 and b['cases'][3]['staged_vertices']==2
 for d in (a,b):
  assert 'File "<ai>", line 2' in d['cases'][3]['traceback']
  assert d['cases'][3]['traceback'].endswith('ValueError: discard-staged-edge\n')
 assert b['scene_clones_per_edge']==0
p=Path(sys.argv[1]);a=json.loads((p/'original.json').read_text());b=json.loads((p/'native.json').read_text());validate(a,b);bad=[]
for name in ['partial-live-mutation','lost-edge-identity','reattach-detached-edge','no-op-history','missing-case']:
 x=copy.deepcopy(b)
 if name=='partial-live-mutation':x['cases'][3]['vertices'].append([8.,0.,0.])
 elif name=='lost-edge-identity':x['cases'][5]['stdout']='False\n'
 elif name=='reattach-detached-edge':x['cases'][7]['stdout']='True\n'
 elif name=='no-op-history':x['cases'][1]['changed']=True
 else:x['cases'].pop()
 try:validate(a,x)
 except AssertionError:bad.append(name)
 else:raise AssertionError('mutation accepted: '+name)
print(json.dumps({'bounded_mesh_and_observable_values_equal':True,'cases':8,'full_traceback_bytes_equal':a['cases'][3]['traceback']==b['cases'][3]['traceback'],'negative_rejections':bad,'all_document_journal':False,'deadline_implemented':False},indent=2))
