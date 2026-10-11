#!/usr/bin/env python3
"""CPU-only operation-order control. This does not emulate either GPU compiler."""
import json,zipfile,sys
from pathlib import Path
import numpy as np
root=Path(__file__).resolve().parents[1]
j=json.loads(zipfile.ZipFile(root/"docs/audits/section-iso-stages/arm.zip").read("native/report.json"))
m=np.array(j["resource"]["camera_column_major_f32_bits"],np.uint32).view(np.float32).T
F=np.float32
# Explicit multiply and left-associated additions, rounding after EACH operation.
def clip_z(p):
 v=np.array(p+[1],np.float32); q=np.float32(m[2]*v)
 return F(F(F(q[0]+q[1])+q[2])+q[3])
def remap(z):return F(F(z+F(1))*F(.5))
def midpoint(a,b):return F(a+F(F(.5)*F(b-a)))
rows=[]
for a,b in [([0,0,1],[1,0,2]),([0,0,1],[0,0,2]),([0,1,1],[0,0,2])]:
 za,zb=clip_z(a),clip_z(b)
 old=remap(midpoint(za,zb)); early=midpoint(remap(za),remap(zb))
 rows.append({"edge":[a,b],"clip_z_endpoint_bits":[int(za.view(np.uint32)),int(zb.view(np.uint32))],"interpolate_then_remap_bits":int(old.view(np.uint32)),"remap_then_interpolate_bits":int(early.view(np.uint32)),"delta_ulps":int(early.view(np.uint32))-int(old.view(np.uint32))})
# Cold literal fixture-domain guard, not a general production admission protocol.
verts=[[x,y,z]for z in[0,1,2]for x,y in[(0,0),(1,0),(1,1),(0,1)]]
clips=[m@np.array(p+[1],np.float32)for p in verts]
assert all(c[3]==F(1) and F(0)<=c[2]<=c[3]for c in clips)
print(json.dumps({"scope":"Explicit scalarf32 sequence, no FMA; not captured hardware VS/clipping values","numpy_version":np.__version__,"all_original_vertices_clip_z_nonnegative_w1":True,"rows":rows},indent=2))
