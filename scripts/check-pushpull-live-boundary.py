"""Independent source/native boundary admission checks; retains literal winding gap."""
import os,sys,json,copy
from pathlib import Path
from collections import Counter
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtGui import QVector3D as V
from core.mesh import Mesh
from core.group import Group
from core import solids
source=json.loads(Path(sys.argv[1]).read_text());native=json.loads(Path(sys.argv[2]).read_text())["cases"]
required={"ctrl-roof-0.2","ctrl-step0.5","ctrl-step-0.5","wedge0--0.4"}
assert len(source)==4 and {r["name"]for r in source}==required
assert len(native)==26 and len({r["name"]for r in native})==26
S={r["name"]:r for r in source};N={r["name"]:r for r in native}
def ring(points,unoriented=False):
 p=[tuple(round(v,5)for v in q)for q in points]
 values=[tuple(p[i:]+p[:i])for i in range(len(p))]
 if unoriented:
  p.reverse();values += [tuple(p[i:]+p[:i])for i in range(len(p))]
 return min(values)
def inventory(fs,ignore_interior_winding=False):
 return sorted((ring(f["vertices"],ignore_interior_winding and f["interior"]),tuple(sorted(ring(h,ignore_interior_winding and f["interior"])for h in f["holes"])),json.dumps(f["attrs"],sort_keys=True),f["interior"])for f in fs)
def manifold(fs):
 m=Mesh()
 for f in fs:
  face=m.add_face([V(*p)for p in f["vertices"]],hole_loops=[[V(*p)for p in h]for h in f["holes"]]);face.interior=f["interior"]
 return solids._to_manifold(Group(m),solids._Sources())
def validate_step(fs):
 assert len(fs)==14 and sum(f["interior"]for f in fs)==2
 assert inventory(fs,True)==inventory(S["ctrl-step0.5"]["polygon_fields"],True)
 expected=manifold(S["ctrl-step0.5"]["polygon_fields"]);actual=manifold(fs)
 difference=(expected-actual).volume()+(actual-expected).volume()
 assert difference<1e-6
 return difference
step=N["ctrl-step0.5"];assert step["error"]is None
assert S["ctrl-step0.5"]["manifold_volume"]==40.5 and not S["ctrl-step0.5"]["triangle_boundary_errors"]
difference=validate_step(step["result"])
for name in ["ctrl-roof-0.2","ctrl-step-0.5"]:
 assert S[name]["manifold_error"]=="not manifold" and S[name]["triangle_boundary_errors"]
 assert N[name]["error"] and N[name]["atomic_on_error"]
wedge=S["wedge0--0.4"];assert wedge["solid_report"][0] and wedge["manifold_error"]=="not manifold"
assert all(e["incidence"]==2 for e in wedge["triangle_boundary_errors"])
assert any(e["oriented_balance"]for e in wedge["triangle_boundary_errors"])
negatives=[]
for kind in ["removed_boundary","reversed_boundary","metadata","interior_marker"]:
 changed=copy.deepcopy(step["result"])
 if kind=="removed_boundary":changed.pop(0)
 elif kind=="reversed_boundary":changed[0]["vertices"].reverse()
 elif kind=="metadata":changed[0]["attrs"]["vendor"]="corrupt"
 else:next(f for f in changed if f["interior"])["interior"]=False
 try:validate_step(changed)
 except (AssertionError,solids.SolidError):negatives.append(kind)
 else:raise AssertionError("corruption accepted: "+kind)
report={"passed":True,"scope":"One corrected valid Ctrl boundary; three live original invalid boundaries retained, not literal-parity closure","ctrl_step_volume":40.5,"spatial_difference":difference,"oriented_inventory_equal":inventory(step["result"])==inventory(S["ctrl-step0.5"]["polygon_fields"]),"interior_winding_ignored_inventory_equal":True,"negative_controls":negatives,"remaining_original_invalid":["ctrl-roof-0.2","ctrl-step-0.5","wedge0--0.4"]}
Path(sys.argv[3]).write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
