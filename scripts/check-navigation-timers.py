#!/usr/bin/env python3
"""Mandatory actual Qt and egui stalled dispatch controls with retained negatives."""
import json,sys,math
from pathlib import Path
source=json.loads(Path(sys.argv[1]).read_text());native=json.loads(Path(sys.argv[2]).read_text());old=json.loads(Path(sys.argv[3]).read_text())
names=['walk-stalled','first-person-stalled']
def check(s,n):
 assert [r['name']for r in s['cases']]==names and [r['name']for r in n['cases']]==names,'mandatory ordered cases'
 for a,b in zip(s['cases'],n['cases']):
  assert .17<=a['blocked_seconds']<2 and a['timeouts']==1 and not a['timer_active'],'actual coalesced timer/cancel'
  assert b['after_start']==b['before'],'no immediate key/press movement'
  assert all(math.isfinite(v)for v in b['delta'])
  # Existing source camera is float32; core navigation is double. This existing
  # one-step arithmetic boundary is <1e-6, distinct from timer-count mismatch.
  assert max(abs(x-y)for x,y in zip(a['delta'],b['delta']))<1e-6,'one source step'
check(source,native)
for i in range(2):
 altered=json.loads(json.dumps(native));altered['cases'][i]=old['cases'][i]
 try:check(source,altered)
 except AssertionError:pass
 else:raise AssertionError('compiled old catch-up defect accepted')
short=json.loads(json.dumps(native));short['cases'].pop()
try:check(source,short)
except AssertionError:pass
else:raise AssertionError('missing first-person accepted')
print(json.dumps(dict(passed=True,mandatory_cases=2,compiled_old_defect_negatives=2,missing_case_negative=True,scope='controlled actual Qt timer dispatch/native egui input; not rendered timing or all-mode acceptance')))
