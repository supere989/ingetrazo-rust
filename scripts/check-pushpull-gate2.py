import sys,os,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QVector3D as V
from core.mesh import Mesh
from core.group import Group
from core import solids
from core.orient import is_closed
s=json.loads(Path("docs/audits/pushpull-gate2/source.json").read_text());n=json.loads(Path(sys.argv[1]).read_text());rows=[]
def mesh(fs):
 m=Mesh()
 for f in fs:
  face=m.add_face([V(*p)for p in f["vertices"]],hole_loops=[[V(*p)for p in h]for h in f["holes"]]);face.interior=f.get("interior",False)
 return m
def area(m):return sum(f.area()for f in m.faces)
assert len(s["cases"])==len(n["cases"])==26
assert len({a["name"] for a in s["cases"]})==26
for a,b in zip(s["cases"],n["cases"]):
 assert a["name"]==b["name"]
 x,y=mesh(a["result"]),mesh(b["result"]);closed=is_closed(x)
 if closed and not b["error"]:
  try:
   mx,my=solids._to_manifold(Group(x),solids._Sources()),solids._to_manifold(Group(y),solids._Sources());diff=(mx-my).volume()+(my-mx).volume();ok=diff<1e-6
  except solids.SolidError as e:diff=str(e);ok=False
 else:
  def vertices(m):return sorted(tuple(round(v,5)for v in[p.x(),p.y(),p.z()])for f in m.faces for p in f.vertices)
  diff=None;ok=not b["error"]and len(x.faces)==len(y.faces)and vertices(x)==vertices(y)
 rows.append({"name":a["name"],"pass":ok,"source_closed":closed,"source_faces":len(x.faces),"native_faces":len(y.faces),"difference":diff,"error":b["error"],"source_status":a["status"]})
print(json.dumps(rows,indent=2));Path(sys.argv[2]).write_text(json.dumps(rows,indent=2)+"\n")

sys.exit(0 if all(row["pass"] for row in rows) else 1)
