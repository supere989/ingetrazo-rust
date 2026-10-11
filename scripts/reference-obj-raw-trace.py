#!/usr/bin/env python3
"""One natural non-manifold raw-region trace refusal; no admission injection."""
import sys,json
sys.path.insert(0,"ingetrazo")
from PySide6.QtGui import QVector3D as V
from core.mesh import Mesh
from formats import fuse,dae
points=[(0,0),(1,0),(0,1),(-1,0),(-1,-1),(1,2)]
rings=[[0,1,2,3],[1,0,4,2,5]]
loops=[]
for i,ring in enumerate(rings):
 attrs={"texture":{"path":"witness.png","uvw":[1.+i*.000001,0.,0.,0.,0.,1.,0.,0.]}}
 loops.append(([V(*points[j],0) for j in ring],attrs))
trace=[]
original_trace=fuse._trace_region
def observe_trace(region):
 result=original_trace(region);trace.append({"region_size":len(region),"refused":result is None});return result
fuse._trace_region=observe_trace
fused=fuse.fuse_coplanar_loops(loops)
assert trace==[{"region_size":2,"refused":True}]
assert len(fused)==2
assert all(len(originals)==1 for _,_,_,originals in fused)
m=Mesh();dae._add_fused(m,fused)
row={"attrs":[f.attrs for f in m.faces],"loops":[[[p.x(),p.y(),p.z()] for p in f.vertices] for f in m.faces]}
open(sys.argv[1],"w").write(json.dumps({"trace":trace,"emitted":row},indent=2)+"\n")
print(json.dumps({"natural_trace_refusal":True,"faces":len(m.faces),"distinct_original_attrs":len({json.dumps(f.attrs,sort_keys=True)for f in m.faces})}))
