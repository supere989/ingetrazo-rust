#!/usr/bin/env python3
"""No GPU/capture: prevalidate the explicit prototype law against frozen sample bits."""
import json,re,struct,zipfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]
with zipfile.ZipFile(root/'docs/audits/section-sample-observation/records.zip')as z:source=json.loads(z.read('mesa/report.json'))
shader=(root/'crates/ingetrazo-metal/src/shaders/world_resolve.metal').read_text();table=shader.split('source_unorm_bits[256]={',1)[1].split('};',1)[0];bits=[int(x,16)for x in re.findall(r'0x([0-9a-f]{8})u',table)];assert len(bits)==256
f=lambda x:struct.unpack('<f',struct.pack('<f',x))[0]
decode=lambda q:struct.unpack('<f',struct.pack('<I',bits[q]))[0]
reciprocal=struct.unpack('<f',struct.pack('<I',0x3b808081))[0]
assert all(bits[q]==struct.unpack('<I',struct.pack('<f',q*reciprocal))[0]for q in range(256))
checked=0
for p in source['pixels']:
 for q,observed in zip(p['sample_rgba8'],p['sample_float_bits']):
  assert [bits[c]for c in q]==observed;checked+=4
 def resolve(mode):
  samples=p['sample_rgba8'];result=[]
  for channel in range(4):
   v=[decode(q[channel])for q in samples]
   summed=f(f(f(v[0]+v[1])+v[2])+v[3]) if mode!='reassociate' else f(f(v[0]+v[1])+f(v[2]+v[3]))
   scaled=f(f(summed*.25)*255)
   result.append(int(scaled)//1 if mode=='truncate' else int(scaled+.5) if mode=='half-away' else round(scaled))
  return result
 assert resolve('ordered')==p['resolved_rgba8']
 p['computed']=resolve('ordered');p['mutations']={m:resolve(m)for m in ['truncate','half-away','reassociate']}
for mutation in ['truncate','half-away','reassociate']:assert any(p['mutations'][mutation]!=p['resolved_rgba8']for p in source['pixels'])
print(json.dumps({'source_channel_float_bits_exact':checked,'decode_entries':256,'ordered_four_pixels_exact':True,'wrong_truncation_half_away_reassociation_rejected':True,'scope':'Scalar proof of measured inputs and explicit operation order; native shader implementation remains pending'},indent=2))
