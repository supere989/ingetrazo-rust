#!/usr/bin/env python3
"""Require actual block upload/readback and complete decoded GPU images."""
import json,sys
from pathlib import Path
BLOCKS={'bc1':bytes((0,248,0,0,0,0,0,0)),'astc':bytes((252,253,255,255,255,255,255,255,255,255,0,0,0,0,255,255))}
EXPECTED=bytes((255,0,0,255))*16
if sys.argv[1:]==['--self-test']:
    for wrong in (b'',bytes(64),bytes((0,0,255,255))*16):assert wrong!=EXPECTED
    print('PASS missing/zero/wrong-color controls; no actualGPU proof')
else:
    root=Path(sys.argv[1]);report=json.loads((root/'result.json').read_text())
    assert set(report)=={'passed','bc1_supported','astc_supported','physical_mac_claim'}
    assert report['passed'] is True and report['physical_mac_claim'] is False
    assert all(type(report[k]) is bool for k in ('bc1_supported','astc_supported'))
    assert report['bc1_supported'] or report['astc_supported'],'no supported compressionGPU proof'
    for name,blocks in BLOCKS.items():
        if report[f'{name}_supported']:
            assert (root/f'{name}.blocks').read_bytes()==blocks,'raw GPU compressed block mismatch'
            assert (root/f'{name}.rgba').read_bytes()==EXPECTED,'decoded GPU pixels mismatch'
        else:
            assert not(root/f'{name}.rgba').exists(),'unsupported format mislabeled GPU proof'
    print('PASS supported compressedGPU producers/decoders; unsupported formats remain explicit')
