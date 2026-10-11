# SPDX-License-Identifier: GPL-3.0-or-later
import json,sys,copy
from pathlib import Path
NAMES=['baked','shared-one','shared-two'];PHASES=['before','grab','preview','center','commit','retype','undo','redo','cancel-preview','cancel']
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text())
def check(value):
 assert s['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [v['kind']for v in s['cases']]==NAMES==[v['kind']for v in value['cases']]
 for a,b in zip(s['cases'],value['cases'],strict=True):
  assert set(a['states'])==set(PHASES)==set(b['states'])
  assert b['registry_consistent'] is True
  for phase in PHASES:assert a['states'][phase]==b['states'][phase]
check(n);neg=[]
for name in ['empty','missing_phase','stale_sibling','merged_equal_mesh','wrong_axes_owner','stale_uv','lost_registry','lost_cancel','double_preview']:
 v=copy.deepcopy(n);c=v['cases'][1]
 if name=='empty':v['cases']=[]
 elif name=='missing_phase':c['states'].pop('center')
 elif name=='stale_sibling':c['states']['preview']['groups'][1]=copy.deepcopy(c['states']['before']['groups'][1])
 elif name=='merged_equal_mesh':c['states']['commit']['groups'][2]=copy.deepcopy(c['states']['commit']['groups'][0])
 elif name=='wrong_axes_owner':c['states']['commit']['groups'][1]['axes']=c['states']['commit']['groups'][0]['axes']
 elif name=='stale_uv':c['states']['commit']['groups'][1]['mesh']['faces'][0]['attrs']['texture']['uvw']=[1]*8
 elif name=='lost_registry':c['registry_consistent']=False
 elif name=='lost_cancel':c['states']['cancel']=copy.deepcopy(c['states']['cancel-preview'])
 else:c['states']['preview']['groups'][0]['mesh']['vertices'][0][0]+=1
 try:check(v)
 except AssertionError:neg.append(name)
 else:raise AssertionError('accepted '+name)
Path(sys.argv[3]).write_text(json.dumps({'exact_source_phases':30,'negative_controls':neg,'native_scope':'Real Targets consumer with snapshot replay; parent owns actual Scale controller/application proof'},indent=2)+'\n')
