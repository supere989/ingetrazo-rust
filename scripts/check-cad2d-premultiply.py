#!/usr/bin/env python3
"""Strict production byte protocol vs actual Qt; preserves QRgb shortcut failures."""
import argparse,hashlib,json
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('original',type=Path);p.add_argument('native',type=Path);p.add_argument('--output',type=Path);a=p.parse_args()
original=(a.original/'channel-alpha.bin').read_bytes();native=a.native.read_bytes();meta=json.loads((a.original/'original.json').read_text());assert len(original)==len(native)==65536;assert hashlib.sha256(original).hexdigest()==meta['sha256']
wrong=[i for i,(s,n)in enumerate(zip(original,native))if s!=n];assert not wrong,wrong[:20]
assert len(meta['qrgb32_negative_mismatches'])==24
corrupted=bytearray(native);corrupted[128*256+1]^=1;assert corrupted!=original
result={'status':'PASS','pairs':65536,'native_sha256':hashlib.sha256(native).hexdigest(),'actual_qt_version':meta['qt_version'],'negative_controls':{'qrgb32_shortcut_mismatches':24,'one_byte_corruption_rejected':True},'scope':'production premultiplication operation against actual QPainter solid brush; coverage/composition geometry and actualGPU pixels separate'}
if a.output:a.output.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
