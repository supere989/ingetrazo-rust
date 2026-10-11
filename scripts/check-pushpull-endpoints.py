import os,sys,json,copy
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtGui import QVector3D as V
from core.mesh import Mesh
from core.group import Group
from core import solids
names=["zero","subminimum","exact-minimum","collapse","past-collapse","painted-collapse","holed-collapse","holed-near-endpoint","bump-flush","corner-through","mouth-through","repeat-after-collapse"]
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text())
assert s["source_pin"]=="6be29fe437a7cd992d4e079f71720821b25216b9"
assert [c["name"]for c in s["cases"]]==names==[c["name"]for c in n["cases"]]
def ring(points):
 q=[tuple(p)for p in points];return min(tuple(q[i:]+q[:i])for i in range(len(q)))
def fields(fs):return sorted((ring(f["vertices"]),tuple(sorted(ring(h)for h in f["holes"])),json.dumps(f["attrs"],sort_keys=True),f["interior"])for f in fs)
def manifold(fs):
 m=Mesh()
 for f in fs:
  face=m.add_face([V(*p)for p in f["vertices"]],hole_loops=[[V(*p)for p in h]for h in f["holes"]]);face.interior=f["interior"]
 return solids._to_manifold(Group(m),solids._Sources())
def validate(a,b):
 assert b["error"]is None
 assert fields(a["result"])==fields(b["result"]),a["name"]+" exact geometry/holes/attrs/interior mismatch"
 if a["name"]in ["zero","subminimum"]:assert b["atomic_on_error"]is True
 if len(a["result"])>1:
  x,y=manifold(a["result"]),manifold(b["result"]);difference=(x-y).volume()+(y-x).volume();assert difference<1e-6;return difference
 assert len(b["result"])==1 and not b["result"][0]["interior"]
 return None
assert next(c for c in s["cases"]if c["name"]=="past-collapse")["clamped_distance"]==-3
rows=[{"name":a["name"],"exact_fields":True,"faces":len(a["result"]),"difference":validate(a,b)}for a,b in zip(s["cases"],n["cases"])]
negatives=[]
for kind,name in [("empty_collapse","collapse"),("duplicate_flat_cap","collapse"),("lost_hole","holed-collapse"),("lost_paint","painted-collapse"),("unclamped_endpoint","past-collapse"),("orientation","corner-through")]:
 a=next(c for c in s["cases"]if c["name"]==name);b=copy.deepcopy(next(c for c in n["cases"]if c["name"]==name))
 if kind=="empty_collapse":b["result"]=[]
 elif kind=="duplicate_flat_cap":b["result"].append(copy.deepcopy(b["result"][0]))
 elif kind=="lost_hole":b["result"][0]["holes"]=[]
 elif kind=="lost_paint":b["result"][0]["attrs"]={}
 elif kind=="unclamped_endpoint":b["result"][0]["vertices"][0][2]=-96
 else:b["result"][0]["vertices"].reverse()
 try:validate(a,b)
 except AssertionError:negatives.append(kind)
 else:raise AssertionError("accepted corruption: "+kind)
app=json.loads(Path(sys.argv[4]).read_text())
assert app["passed"]is True
required=names+["nested-clamped-collapse-repeat","wider-punchable-sheet","annular-support"]
assert [c["case"]for c in app["cases"]]==required
for row in app["cases"]:
 for value in row.values():
  if isinstance(value,bool):assert value is True
nested=next(c for c in app["cases"]if c["case"]=="nested-clamped-collapse-repeat")
assert nested["depth"]==2 and nested["flat_counts"]==[1,4,4]
assert nested["local_undo_steps"]==2 and nested["folded_document_undo_steps"]==1
assert [c["name"]for c in s["limit_controls"]]==["wider-punchable-sheet","annular-support"]
for control in s["limit_controls"]:
 actual=next(c for c in app["cases"]if c["case"]==control["name"])
 assert actual["clamped_distance"]==control["clamped_distance"]
 assert actual["source_limit_exact"]is True and actual["preview_model_exact"]is True
report={"passed":True,"cases":rows,"negative_controls":negatives,"app_cases":required,"scope":"12 exact source geometric/attribute inventories; closed shape oracle tolerance unchanged1e-6"}
Path(sys.argv[3]).write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
