#!/usr/bin/env python3
"""Causal source-operation raster diagnostic; not a production renderer."""
import argparse, hashlib, json
from pathlib import Path
import numpy as np
from PIL import Image
p=argparse.ArgumentParser()
p.add_argument('source',type=Path);p.add_argument('output',type=Path)
a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
ops=json.loads((a.source/'annotations-on-operations.json').read_text())['actual_operations']
base=np.asarray(Image.open(a.source/'annotations-off.png').convert('RGBA')).copy()
expected=np.asarray(Image.open(a.source/'geometry-widget-diagnostic.png').convert('RGBA'))
assert base.shape==expected.shape==(600,800,4)
counts={};vertices_total=0
for op in ops:
 kind=op['kind']
 if kind=='text':continue
 counts[kind]=counts.get(kind,0)+1
 if kind=='line':
  v0=np.asarray(op['a'],dtype=np.float32);v1=np.asarray(op['b'],dtype=np.float32)
  delta=v1-v0;length=np.hypot(*delta)
  if length==0:raise ValueError('degenerate source line requires independent policy')
  pw=np.float32(np.float32(op['width'])/2)/length
  nx=np.float32(-delta[1]*pw);ny=np.float32(delta[0]*pw)
  n=np.array([nx,ny],dtype=np.float32);cap=np.array([-ny,nx],dtype=np.float32)
  vertices=np.array([v0+cap+n,v0+cap-n,v1-cap-n,v1-cap+n],dtype=np.float32)
  color=op['pen']
 elif kind=='polygon':vertices=np.asarray(op['points'],dtype=np.float32);color=op['brush']
 else:raise ValueError('unsupported diagnostic primitive '+kind)
 assert color[3]==255 and np.isfinite(vertices).all()
 vertices_total+=len(vertices)
 x0,y0=np.maximum(np.floor(vertices.min(axis=0)).astype(int),0)
 x1,y1=np.minimum(np.ceil(vertices.max(axis=0)).astype(int),[800,600])
 for y in range(y0,y1):
  for x in range(x0,x1):
   point=np.array([x+.5,y+.5],dtype=np.float64)
   cross=[]
   for i,v in enumerate(vertices):
    d=vertices[(i+1)%len(vertices)]-v;q=point-v
    cross.append(d[0]*q[1]-d[1]*q[0])
   if all(t>=0 for t in cross) or all(t<=0 for t in cross):base[y,x]=color
changed=np.any(base!=expected,axis=2)
result={'scope':'opaque source-operation causal diagnostic only; no GPU/top-left/degenerate/DPR generalization','operations':counts,'vertices':vertices_total,'pixels':int(changed.sum()),'max':int(np.abs(base.astype(int)-expected.astype(int)).max()),'source_sha256':{f:hashlib.sha256((a.source/f).read_bytes()).hexdigest() for f in ['annotations-on-operations.json','annotations-off.png','geometry-widget-diagnostic.png']}}
Image.fromarray(base).save(a.output/'reconstructed.png')
(a.output/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result));raise SystemExit(int(result['pixels']!=0))
