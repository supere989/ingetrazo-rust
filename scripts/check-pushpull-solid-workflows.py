"""Exact source fields and actual App/history gates; no spatial tolerance."""
import copy,json,sys,hashlib
from pathlib import Path
root=Path(sys.argv[1]);source=json.loads((root/"source.json").read_text());native=json.loads((root/"native.json").read_text())
app_input=json.loads((root/"app-input.json").read_text());app=json.loads(Path(sys.argv[3]).read_text()) if len(sys.argv)>3 else json.loads((root/"app-native.json").read_text())
pinned_sources={'source.json': '159381922f79316805da668151180acf7276e9c02800eaf103d789588302c7a5', 'operative-holed-source.json': 'de2f10b11a18520065ae1788ef243a3e6559be3cd581d15f80f6403ff339298b', 'app-input.json': '135b1d0d7debcb5210ec3e88f1ae5a271a99f4163fce7025be2202c0580c65d2'}
for name,digest in pinned_sources.items():assert hashlib.sha256((root/name).read_bytes()).hexdigest()==digest
required=["test_repush_top_extends_holed_wall_keeping_opening-"+str(i) for i in range(3)]+["test_stacked_block_side_pushes_stay_clean-"+str(i) for i in range(4)]+["test_push_through_wall_makes_a_real_hole-"+str(i) for i in range(2)]
required_app=[required[5],required[6],required[8],"operative-holed-wall-repush","ctrl-step0.5","nested-shared-stacked-wall"]
assert [r["name"]for r in source["cases"]]==required
assert [r["name"]for r in native["cases"]]==required
assert [r["name"]for r in app_input["cases"]]==required_app
assert [r["name"]for r in app["cases"]]==required_app
assert len(source["source_tests"])==3 and all(r["source_test_error"] is None for r in source["source_tests"])
def ring(ps):
 return min(tuple(tuple(p)for p in ps[i:]+ps[:i]) for i in range(len(ps)))
def inventory(fs):
 return sorted((ring(f["vertices"]),tuple(sorted(ring(h)for h in f["holes"])),json.dumps(f["attrs"],sort_keys=True),f["interior"])for f in fs)
def translated(fs,offset):
 out=copy.deepcopy(fs)
 for f in out:
  for r in [f["vertices"],*f["holes"]]:
   for p in r:
    for i in range(3):p[i]+=offset[i]
 return out
operative=json.loads((root/"operative-holed-source.json").read_text())["cases"][0]
for s,n in zip(source["cases"],native["cases"]):
 assert n["error"] is None
 if s["name"]==required[2]:
  assert inventory(s["result"])!=inventory(n["result"])
  assert inventory(n["result"])==inventory(operative["result"])
 else:assert inventory(s["result"])==inventory(n["result"]),s["name"]
def validate_app(s,n):
 expected=translated(s["result"],[6.,8.,6.]) if s.get("nested") else s["result"]
 assert n["error"] is None and n["commit_undo"]==1 and n["acquisition_undo"]==0
 assert n["cancel_exact"] and n["undo_exact"] and n["redo_exact"]
 assert inventory(n["result"])==inventory(expected)
 assert inventory(n["result"])==inventory(n["redo"])
 assert inventory(n["acquired"])==inventory(n["undo"])==inventory(n["cancel"])
 assert n["nested"]==bool(s.get("nested"))
 if s.get("nested"):assert n["linked_finish"]
for s,n in zip(app_input["cases"],app["cases"]):validate_app(s,n)
gate=json.loads((root/"gate2-native.json").read_text())["cases"]
assert len(gate)==26 and len({c["name"]for c in gate})==26
for name in ["ctrl-roof-0.2","ctrl-step-0.5"]:
 row=next(r for r in gate if r["name"]==name);assert row["error"] and row["atomic_on_error"]
wedge=next(r for r in gate if r["name"]=="wedge0--0.4")
original_gate=json.loads((root.parent/"pushpull-gate2/source.json").read_text())["cases"]
wedge_input=next(r for r in original_gate if r["name"]=="wedge0--0.4")
assert wedge["error"] is None and wedge["atomic_on_error"]
assert inventory(wedge["result"])==inventory(wedge_input["source"])
valid=next(r for r in gate if r["name"]=="ctrl-step0.5")
assert valid["error"] is None and inventory(valid["result"])==inventory(app_input["cases"][4]["result"])
negatives=[]
for name in ["empty","missing_case","duplicate_case","removed_face","reversed_boundary","wrong_interior","wrong_paint","lost_hole","coordinate_drift","wrong_undo","lost_cancel","missing_history","lost_linked_finish"]:
 rows=copy.deepcopy(app["cases"])
 if name=="empty":rows=[]
 elif name=="missing_case":rows.pop()
 elif name=="duplicate_case":rows[-1]=rows[0]
 elif name=="removed_face":rows[0]["result"].pop()
 elif name=="reversed_boundary":rows[0]["result"][0]["vertices"].reverse()
 elif name=="wrong_interior":rows[0]["result"][0]["interior"]=True
 elif name=="wrong_paint":rows[0]["result"][0]["attrs"]["vendor"]="wrong"
 elif name=="lost_hole":next(f for f in rows[0]["result"] if f["holes"])["holes"]=[]
 elif name=="coordinate_drift":rows[0]["result"][0]["vertices"][0][0]+=2**-22
 elif name=="wrong_undo":rows[0]["undo"]=rows[0]["result"]
 elif name=="lost_cancel":rows[0]["cancel_exact"]=False
 elif name=="missing_history":rows[0]["commit_undo"]=0
 else:rows[-1]["linked_finish"]=False
 try:
  assert [r["name"]for r in rows]==required_app
  for s,n in zip(app_input["cases"],rows):validate_app(s,n)
 except (AssertionError,KeyError):negatives.append(name)
 else:raise AssertionError("accepted corrupt record "+name)
report={"passed":True,"scope":"Eight exact pinned private regression operations; six exact actual App consumers including nested shared history. Direct helper holed-winding discrepancy retained; no full P03 promotion.","exact_model_cases":8,"actual_app_cases":6,"source_private_helper_discrepancy":required[2],"negative_controls":negatives,"retained_invalid_original_atomic_refusals":["ctrl-roof-0.2","ctrl-step-0.5"],"retained_wedge_policy":"Unchanged zero-limit no-op; literal invalid-source result not copied"}
Path(sys.argv[2]).write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
