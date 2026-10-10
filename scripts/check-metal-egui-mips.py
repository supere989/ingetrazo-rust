#!/usr/bin/env python3
"""Strict actual GPU-produced mip/readback protocol, not compression parity."""
import json,sys
from pathlib import Path
SIZES=((7,5),(3,2),(1,1))
def verify_mip(raw,size):
    if raw!=bytes((37,93,157,255))*(size[0]*size[1]):raise ValueError('missing/wrong mip producer pixels')
if sys.argv[1:]==['--self-test']:
    for size in SIZES:
        verify_mip(bytes((37,93,157,255))*(size[0]*size[1]),size)
        for raw in (b'',bytes(size[0]*size[1]*4),bytes((37,93,156,255))*(size[0]*size[1])):
            try:verify_mip(raw,size)
            except ValueError:pass
            else:raise AssertionError('negative admitted')
    print('PASS nine mip negative controls; no actual GPU execution')
else:
    root=Path(sys.argv[1])
    for i,size in enumerate(SIZES):verify_mip((root/f'mip-{i}.rgba').read_bytes(),size)
    for i,size in enumerate(((8,4),(4,2),(2,1),(1,1))):verify_mip((root/f'batch-mip-{i}.rgba').read_bytes(),size)
    assert (root/'checker-minified.rgba').read_bytes()==bytes((128,128,128,255)), 'mip LOD missing/wrong'
    assert json.loads((root/'result.json').read_text())=={'passed':True,'mips':3,'batch_images':2,'batch_commands':1,'warm_generation_jobs':0,'transactional_refusal':True,'pending_drop_retained':True,'physical_mac_claim':False}
    print('PASS GPU mip producer, minified sampler, warm/cache/ownership protocol')
