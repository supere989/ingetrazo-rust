#!/usr/bin/env python3
"""Source-backed eye boundary controls, additional to unchanged perspective gate."""
import runpy,json,subprocess,hashlib
from pathlib import Path
namespace=runpy.run_path(str(Path(__file__).with_name('check-python-perspective-screen.py')))
Stub,V,arr,step,q,compute_snap,N=(namespace[k] for k in ['Stub','V','arr','step','q','compute_snap','N'])
v=Stub();eye=v.camera.eye();forward=v.camera.forward();front=eye+forward*.05;back=eye-forward*.5
cases=[]
for depth in [.001,.099,.1,.101,.5]:
 point=eye+forward*depth
 cases.append(dict(name=f"held-out-depth-{depth}",edges=[[arr(point),arr(point+V(.1,0,0))]],steps=[step(v,point)],camera=v.camera_json()))
cases.append(dict(name="held-out-behind-eye",edges=[[arr(back),arr(back+V(.1,0,0))]],steps=[step(v,front)],camera=v.camera_json()))
native=json.loads(subprocess.check_output([str(namespace['native_binary'].resolve())],input=json.dumps(cases).encode()))
assert isinstance(native,list) and len(native)==len(cases)
rows=[]
for case,result in zip(cases,native):
 assert result['name']==case['name'] and len(result['steps'])==1
 pixel_step=case['steps'][0]
 snap=compute_snap(q(pixel_step['candidate']),pixel_step['pixel'],N(edges=[N(a=q(a),b=q(b),center=False) for a,b in case['edges']]),v._world_to_pixel,9,project_onto_line=lambda a,d:v._project_to_lock_line(a,d,*pixel_step['pixel']),edge_threshold_px=14,is_occluded=v._is_occluded)
 expected=dict(kind=snap.kind,point=arr(snap.point));actual=result['steps'][0]
 assert expected['kind']==actual['kind'] and len(actual['point'])==3
 assert max(abs(x-y) for x,y in zip(expected['point'],actual['point']))<1e-4,(expected,actual)
 rows.append(dict(input=case,original=expected,native=actual))
assert rows[-1]['native']['kind']=='none','Behind-eye target must reject'
assert all(row['native']['kind']=='endpoint' for row in rows[:-1]),'Positive depth targets must snap'
report=dict(context=namespace['context'](),native_sha256=hashlib.sha256(namespace['native_binary'].read_bytes()).hexdigest(),passed=True,cases=rows,scope='Six independent held-out near-plane depth controls plus unchanged eleven-case perspective gate; source actual Qt projection and empty-face occlusion method; no renderer clipping changes or independent UI claim')
Path('docs/audits/python-snap-heldout-depth-evidence.json').write_text(json.dumps(report,indent=2)+'\n')
print('Held-out eye depth controls 6/6 PASS')
