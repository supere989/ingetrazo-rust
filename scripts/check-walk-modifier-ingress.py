#!/usr/bin/env python3
import json,sys,math,copy
from pathlib import Path
s,n=[json.loads(Path(p).read_text())for p in sys.argv[1:]]
assert [r['name']for r in s['cases']]==['source-no-control','source-control-released-after-motion']
assert [r['name']for r in n['cases']]==['final-state-only-negative','native-ingress-same-frame']
assert all(not r['global_control']for r in s['cases'])and [r['remembered_control']for r in s['cases']]==[False,True]
assert all(r['frame_final_ctrl']==False and r['emitted_egui_events']==4 for r in n['cases'])
def same(a,b):
 assert a['undo']==b['undo']==0
 assert all(math.isfinite(y)and abs(x-y)<=1e-6 for k in ['eye','forward']for x,y in zip(a[k],b[k])),'unchanged single-step Qt/native bound'
same(s['cases'][0],n['cases'][0]);same(s['cases'][1],n['cases'][1])
try:same(s['cases'][1],n['cases'][0])
except AssertionError:pass
else:raise AssertionError('compiled final-state-only path accepted as source Ctrl hover')
print(json.dumps(dict(passed=True,source_handler_cases=2,native_ingress_cases=2,final_state_only_negative_rejected=True,scope='actual original viewport QMouse/QKey handlers and actual egui-winit State native owned-window ingress with controlled events; no real coalescing/GPU/Metal/Glow parity claim')))
