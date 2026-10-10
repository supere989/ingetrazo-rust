#!/usr/bin/env python3
"""Independent strict egui font/alpha/sampler constant-pixel contract."""
import json,sys
from pathlib import Path
SHADES=(255,136,81,37,0,127,128,255,0,255)
EXPECTED=b''.join(bytes((s,s,s,255))*8 for s in SHADES)*8

def check(data):
    if len(data)!=80*8*4 or data!=EXPECTED: raise ValueError("strict full-image egui contract mismatch")

if sys.argv[1:]==['--self-test']:
    check(EXPECTED)
    for raw in (b'',bytes(len(EXPECTED)),bytes([255])*len(EXPECTED),EXPECTED[::-1]):
        try:check(raw)
        except ValueError:pass
        else:raise AssertionError('negative admitted')
    print('PASS four malformed/missing/black/wrong-origin negatives; no GPU execution')
else:
    root=Path(sys.argv[1]);check((root/'contract.rgba').read_bytes());check((root/'expected.rgba').read_bytes())
    assert json.loads((root/'result.json').read_text())=={'passed':True,'cases':10,'warm_gpu_images':5,'physical_mac_claim':False}
    print('PASS ten strict whole-image generic egui controls')
