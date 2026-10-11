# SPDX-License-Identifier: GPL-3.0-or-later
import os,sys,json
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtGui import QVector3D as V
from core.mesh import Mesh
from core.topology import fold_nonplanar_faces
recipes=[('lifted-quad',[[0,0,0],[4,0,0],[4,4,1],[0,4,0]],[]),('gentle-hex',[[0,0,0],[2,0,0],[4,0,0],[4,4,.05],[2,4,.05],[0,4,0]],[]),('warped-hole',[[0,0,0],[6,0,0],[6,6,1],[0,6,0]],[[[2,2,0],[2,4,0],[4,4,0],[4,2,0]]]),('planar-hole',[[0,0,0],[6,0,0],[6,6,0],[0,6,0]],[[[2,2,0],[2,4,0],[4,4,0],[4,2,0]]])]
def vec(v):return [v.x(),v.y(),v.z()]
def snap(m):return [dict(vertices=[vec(v)for v in f.vertices],holes=[[vec(v)for v in h]for h in f.holes],attrs=dict(f.attrs),interior=f.interior)for f in m.faces]
rows=[]
for name,outer,holes in recipes:
 m=Mesh();f=m.add_face([V(*p)for p in outer],hole_loops=[[V(*p)for p in h]for h in holes]);f.attrs={'color':[.2,.4,.6],'vendor':71}
 neighbor=m.add_face([V(10,0,0),V(11,0,0),V(11,1,0)]);neighbor.attrs={'untouched':True};source=snap(m);oldedges=list(m.edges);folded=fold_nonplanar_faces(m)
 rows.append(dict(name=name,source=source,result=snap(m),folded=len(folded),untouched_identity=neighbor in m.faces,original_edges_survive=all(e in m.edges for e in oldedges)))
open(sys.argv[1],'w').write(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','cases':rows},indent=2)+'\n')
