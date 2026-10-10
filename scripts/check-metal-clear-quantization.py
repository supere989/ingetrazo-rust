#!/usr/bin/env python3
"""Strict native GPU clear controls; fractional conversion is measured, not fitted."""
import json, sys
from pathlib import Path
root=Path(sys.argv[1])
r=json.loads((root/'result.json').read_text())
assert r['passed'] is True and r['gpu_to_host_readback'] is True
assert r['target_format']=='BGRA8Unorm'
assert r['windowserver_color_processing'] is False and r['drawable_readback_claim'] is False
assert r['invalid_clear_negative'] is True and r['final_ledger']==0
names=['actual-fractional-clear','exact-byte-clear','black-control','white-control']
assert [c['name'] for c in r['cases']]==names
for c in r['cases']:
    data=(root/(c['name']+'.rgba')).read_bytes()
    pixel=bytes(c['raw_gpu_rgba'])
    assert len(pixel)==4 and len(data)==16384 and data==pixel*4096
    expected={'exact-byte-clear':bytes([20,51,89,255]),'black-control':bytes([0,0,0,255]),'white-control':bytes([255]*4)}.get(c['name'])
    if expected is not None: assert pixel==expected
print('PASS four native GPU clear/readback controls; fractional value retained without fitted expectation')
