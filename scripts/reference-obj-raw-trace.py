#!/usr/bin/env python3
"""One natural non-manifold raw-region trace refusal; no admission injection."""
import sys,json
sys.path.insert(0,"ingetrazo")
from PySide6.QtGui import QVector3D as V
from core.mesh import Mesh
from formats import fuse,dae
loops=[]
for i,y in enumerate([1.,2.,-1.]):
 attrs={"texture":{"path":"witness.png","uvw":[1.+i*.000001,0.,0.,0.,0.,1.,0.,0.]}}
 loops.append(([V(0,0,0),V(1,0,0),V(.5,y,0)],attrs))
fused=fuse.fuse_coplanar_loops(loops)
assert len(fused)==3
assert all(len(originals)==1 for _,_,_,originals in fused)
m=Mesh();dae._add_fused(m,fused)
row={"attrs":[f.attrs for f in m.faces],"loops":[[[p.x(),p.y(),p.z()] for p in f.vertices] for f in m.faces]}
open(sys.argv[1],"w").write(json.dumps(row,indent=2)+"\n")
print(json.dumps({"natural_trace_refusal":True,"faces":len(m.faces),"distinct_original_attrs":len({json.dumps(f.attrs,sort_keys=True)for f in m.faces})}))
