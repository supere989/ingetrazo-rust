#!/usr/bin/env python3
"""Compare frozen actual Qt inputs with freshly rendered production World output."""
import json,sys
from pathlib import Path
original=json.loads(Path(sys.argv[1]).read_text())['cases']
native=json.loads(Path(sys.argv[2]).read_text())['cases']
baseline=json.loads(Path(sys.argv[3]).read_text())['cases']
assert len(original)==len(native)>=10 and len(baseline)>=10
for o,n in zip(original,native):
 assert o['case']==n['case']
 assert o['center_rgba']==n['center_rgba'],(o['case'],o['center_rgba'],n['center_rgba'])
 expected=[(r[0],r[2],r[3]) for r in o['cached_opacity_runs']]
 assert len(expected)==len(n['opacity_runs']), (o['case'],expected,n['opacity_runs'])
 first=0
 for e,a in zip(expected,n['opacity_runs']):
  # Original offsets include the opaque color buffer prefix; native owns only transparent positions/colors.
  assert abs(e[0]-a[0])<1e-6 and e[2]==a[2] and a[1]==first,(o['case'],e,a)
  first+=a[2]
 assert n['geometry_uploads']==1
 assert n['surface_bytes']==sum(r[2] for r in n['opacity_runs'])*24
 assert 'additional_position_bytes: 0,' in n['contour_metrics']
assert sum(o['center_rgba']!=b['center_rgba'] for o,b in zip(original,baseline))>=8
# A reversed same-opacity draw order and a linear-light blend must be rejected.
rows={n['case']:n for n in native}
assert rows['equal-opacity-forward']['center_rgba']!=rows['equal-opacity-reversed']['center_rgba']
assert rows['red-half-black']['center_rgba'][0]==124 and rows['red-half-black']['center_rgba'][0]!=183
print(f'PASS {len(original)} exact actual Qt alpha pixels, cached bucket order, 24B shared vertices, no duplicate positions, stale opaque/linear/order negative controls')
