"""Independent pinned Python manifold reader for every retained solid output."""
import sys,os,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtGui import QVector3D as V
from core.mesh import Mesh
from core.group import Group
from core import solids
root=Path(sys.argv[1]);S=json.loads((root/"source.json").read_text())["cases"];N=json.loads((root/"native.json").read_text())["cases"]
def shape(fs):
 m=Mesh()
 for f in fs:
  row=m.add_face([V(*p)for p in f["vertices"]],hole_loops=[[V(*p)for p in h]for h in f["holes"]]);row.interior=f["interior"]
 return solids._to_manifold(Group(m),solids._Sources())
rows=[]
for s,n in zip(S,N):
 assert s["name"]==n["name"] and n["error"] is None
 a,b=shape(s["result"]),shape(n["result"])
 delta=(a-b).volume()+(b-a).volume()
 assert delta==0.0 and a.volume()==b.volume(),s["name"]
 rows.append({"name":s["name"],"source_volume":a.volume(),"native_volume":b.volume(),"symmetric_difference":delta})
Path(sys.argv[2]).write_text(json.dumps({"passed":True,"scope":"Independent original manifold reader; exact physical shapes, does not normalize the separate oriented metadata gate.","cases":rows},indent=2)+"\n")
print(len(rows),"exact source/native manifold shapes")
