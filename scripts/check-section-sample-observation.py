#!/usr/bin/env python3
"""Validate complete diagnostic observations, without accepting release parity."""
import json,math,struct,sys
from pathlib import Path
root=Path(sys.argv[1]);report=json.loads((root/'report.json').read_text());cases=report['cases'];assert len(cases)==1 and cases[0]['case']=='closed'
case=cases[0];assert case['sample_count']==4 and case['pixel_format']=='RGBA8Unorm'
points=[[400,228],[328,300],[471,300],[400,371]];assert [p['point_top_origin']for p in case['pixels']]==points
raw=(root/'closed.rgba').read_bytes();assert len(raw)==800*600*4
for p in case['pixels']:
 x,y=p['point_top_origin'];assert p['resolved_rgba8']==list(raw[(y*800+x)*4:(y*800+x)*4+4])
 assert len(p['sample_float'])==len(p['sample_float_bits'])==len(p['sample_rgba8'])==4
 for values,bits,encoded in zip(p['sample_float'],p['sample_float_bits'],p['sample_rgba8']):
  assert len(values)==len(bits)==len(encoded)==4
  for value,bit,byte in zip(values,bits,encoded):
   assert type(value)in(float,int) and math.isfinite(value) and 0<=value<=1
   assert type(bit)is int and struct.unpack('<I',struct.pack('<f',value))[0]==bit
   assert type(byte)is int and abs(value*255-byte)<1e-4
print(json.dumps({'complete':True,'pixels':4,'samples':16,'scope':'Observation completeness only; source/Metal color and resolve acceptance pending independent comparison'},indent=2))
