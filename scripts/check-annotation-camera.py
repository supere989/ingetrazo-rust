#!/usr/bin/env python3
"""Strict real-camera matrix comparison; no injected matrices or tolerance."""
import json,sys,struct
from pathlib import Path
source,native=map(lambda p:json.loads(Path(p).read_text()),sys.argv[1:3])
names=['perspective','ortho','two-point','top','near-eye','translated','gondola_v20-iso','gondola_v20-asymmetric','capilla_quiroz_v17-iso','capilla_quiroz_v17-asymmetric']
assert source['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
assert [x['name']for x in source['cases']]==names
assert [x['name']for x in native['cases']]==names
def bits(v):return struct.pack('<f',v)
def check(a,b):
 for field in ['projection','view','matrix','inverse','eye','forward']:
  assert len(a[field])==len(b[field])
  assert all(x==y for x,y in zip(a[field],b[field])),(a['name'],field)
  if field in ['matrix','inverse']:assert all(bits(x)==bits(y)for x,y in zip(a[field],b[field])),(a['name'],field,'bits')
old=[]
for a,b in zip(source['cases'],native['cases']):
 check(a,b);n=sum(bits(x)!=bits(y)for x,y in zip(a['matrix'],b['old_matrix']));assert n>0;old.append(n)
mut=json.loads(json.dumps(native['cases'][0]));mut['matrix'][0]+=0.01
try:check(source['cases'][0],mut)
except AssertionError:pass
else:raise AssertionError('altered matrix admitted')
print(json.dumps({'cases':10,'mvp_inverse_float32_bits_exact':True,'all_values_exact':True,'view_signed_zero_only':True,'old_matrix_differences':old,'altered_matrix_rejected':True}))
