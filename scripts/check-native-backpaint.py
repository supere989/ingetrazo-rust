#!/usr/bin/env python3
"""Literal original Qt backpaint/front-cull/gate/order protocol."""
import json,sys
from pathlib import Path
o=json.loads(Path(sys.argv[1]).read_text())['cases'];n=json.loads(Path(sys.argv[2]).read_text())['cases'];old=json.loads(Path(sys.argv[3]).read_text())['cases']
base=["red-half-white","red-half-black","opacity-bucket-order","opaque-front","opaque-back","equal-opacity-forward","equal-opacity-reversed","opacity-995","opacity-zero","section-clipped"]
back=[f"back-{f}-{b}-{direction}" for f,b in [(0.5,0.3),(1.,0.3),(0.5,1.),(1.,1.)] for direction in ["front","reversed"]]+["back-gate-activated-front","back-gate-activated-reversed","back-order-translucent-opaque","back-order-opaque-translucent"]
assert [r["case"] for r in o]==base+back==[r["case"] for r in n]
assert [r["case"]for r in old]==base+back
assert len(o)==len(n)==len(old)==22
for a,b in zip(o,n):
 assert a['case']==b['case'] and a['center_rgba']==b['center_rgba'],(a['case'],a['center_rgba'],b['center_rgba'])
 for source,native in [(a['cached_opacity_runs'],[r for r in b['paint_runs'] if r[0] in (0,1)]),(a['cached_back_opacity_runs'],[r for r in b['paint_runs'] if r[0]==2])]:
  assert len(source)==len(native),(a['case'],source,native)
  for x,y in zip(source,native):
   count=x[3] if len(x)==4 else x[2]
   assert abs(x[0]-y[1])<1e-6 and count==y[3],(a['case'],x,y)
   if len(x)==4:assert int(x[1])==y[0]
 assert sum(r[3] for r in b['paint_runs'] if r[0]==3)==a['opaque_back_run'][1]
 assert b.get('consolidated_opaque_count',sum(r[1] for r in b['ordinary_runs'] if not r[2]))==a['ordinary_faces_count']
 assert sum(r[1] for r in b['ordinary_runs'] if r[2])==a['front_culled_opaque_run'][1]
 assert b['surface_bytes']==24*sum(r[3] for r in b['paint_runs']) and b['geometry_uploads']==1
 assert 'additional_position_bytes: 0,' in b['contour_metrics']
old_by_name={r['case']:r for r in old}
assert sum(max(abs(x-y) for x,y in zip(a['center_rgba'],old_by_name[a['case']]['center_rgba']))>8 for a in o if a['case'] in old_by_name)>=5
rows={x['case']:x for x in n}
if 'back-gate-instanced-only-front' in rows:
 row=rows['back-gate-instanced-only-front']
 assert row['consolidated_opaque_count']==0 and row['center_rgba']==[255,255,255,255]
 assert any(not r[2] and r[1]>0 for r in row['ordinary_runs']), 'Previous flattened ordinary-face gate failure was not exercised'
assert rows['back-1.0-0.3-front']['center_rgba']==[255,255,255,255]
assert rows['back-gate-activated-front']['center_rgba']=={x['case']:x for x in o}['back-gate-activated-front']['center_rgba']
assert rows['back-0.5-0.3-front']['center_rgba']!=rows['back-0.5-0.3-reversed']['center_rgba']
print(f'PASS {len(o)} exact original/native RGBA, front/back runs, opaque gate, shared24B/cached geometry; retained missing-backpaint and gate/culling negative controls')
