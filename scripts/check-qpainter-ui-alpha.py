#!/usr/bin/env python3
import json,sys
from pathlib import Path
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text())
assert len(s['phases'])==2 and len(n['phases'])==2
names=['solid-red64-black','solid-red128-black','solid-red192-black','solid-rgb128-white','solid-white128-black','solid-blue128-gray','opaque-rgb','opaque-nearest-checker','alpha-nearest-checker','alpha-texture-tint128','partial-opaque-checker','partial-alpha-checker']
rows=[]
for phase,actual in zip(['baseline','partial'],n['phases']):
 assert actual['phase']==phase
 assert [c['case']for c in actual['cases']]==names
 assert [c['case']for c in s['phases'][phase]]==names
 for ref,row in zip(s['phases'][phase],actual['cases']):
  assert len(ref['rgba'])==len(row['pixels'])==4
  rows.append({'phase':phase,'case':row['case'],'expected':ref['rgba'],'actual':row['pixels'],'passed':ref['rgba']==row['pixels']})
result={'passed':all(c['passed']for c in rows),'cases':rows,'failed':sum(not c['passed']for c in rows),'scope':'Actual QPainter raster versus actual production NVIDIA Vulkan UI; strict byte comparison'}
Path(sys.argv[3]).write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'passed':result['passed'],'failed':result['failed'],'total':len(rows)}))
sys.exit(0 if result['passed']else 1)
