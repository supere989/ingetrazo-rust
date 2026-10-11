import copy,json,sys
from pathlib import Path
names=["diagonal-protected","ring-island-protected","annular-divisions","diagonal-wires","ring-island-empty","annular-tjunction-invalid"]
s=json.loads(Path(sys.argv[1]).read_text());n=json.loads(Path(sys.argv[2]).read_text())
assert s["source_pin"]=="6be29fe437a7cd992d4e079f71720821b25216b9"
assert [c["name"]for c in s["cases"]]==names==[c["name"]for c in n["cases"]]
def ring(points):
 q=[tuple(p)for p in points];return min(tuple(q[i:]+q[:i])for i in range(len(q)))
def fields(fs):return sorted((ring(f["vertices"]),tuple(sorted(ring(h)for h in f["holes"])),json.dumps(f["attrs"],sort_keys=True),f["interior"])for f in fs)
def vertices(m):return sorted((tuple(p),-1 if origin is None else origin)for p,origin in zip(m["vertices"],m["vertex_origins"],strict=True))
def edges(m):return sorted((tuple(sorted(tuple(p)for p in e["ends"])),e["soft"],e["hidden"],-1 if e["curve"]is None else e["curve"],e["layer"]or"",e["incidence"])for e in m["edges"])
def same(a,b):
 assert fields(a["faces"])==fields(b["faces"]),"face/hole/orientation/paint winner mismatch"
 assert vertices(a)==vertices(b),"vertex/identity mismatch"
 assert edges(a)==edges(b),"edge/flags/incidence mismatch"
def validate(a,b):
 assert b["preview_cancel_exact"]is True and b["app_vcb_undo_redo_exact"]is True
 assert a["clamped_distance"]==-3 and a["undo"]==1
 assert a["source_undo_exact"]is True and a["source_redo_exact"]is True
 if a["name"]=="annular-tjunction-invalid":
  assert any(len({tuple(p)for p in f["vertices"]})<len(f["vertices"])for f in a["result"]["faces"])
  assert b["error"]and "degenerate supporting face"in b["error"]
  assert b["atomic_on_error"]is True;same(a["source"],b["result"])
 else:
  assert b["error"]is None;same(a["result"],b["result"])
rows=[]
for a,b in zip(s["cases"],n["cases"],strict=True):
 validate(a,b)
 rows.append({"name":a["name"],"exact_fields_and_vertex_ownership":a["name"]!="annular-tjunction-invalid","faces":len(b["result"]["faces"]),"vertices":len(b["result"]["vertices"]),"edges":len(b["result"]["edges"]),"source_raw_incidence_excess":sum(e["raw_incidence"]-e["incidence"]for e in a["result"]["edges"]),"atomic_invalid_output_refusal":bool(b["error"])})
assert n["nested"]=={"depth":2,"integer_affine":True,"local_history_exact":True,"shared_publication_exact":True,"folded_document_history_exact":True}
neg=[]
for kind,name in [("lost_partition","diagonal-protected"),("wrong_winner","ring-island-protected"),("lost_empty_fallback","ring-island-empty"),("lost_hole","annular-divisions"),("lost_wire","diagonal-wires"),("lost_wire_flags","diagonal-wires"),("changed_vertex_owner","diagonal-protected"),("wrong_winding","diagonal-protected"),("accepted_invalid_output","annular-tjunction-invalid")]:
 a=next(c for c in s["cases"]if c["name"]==name);b=copy.deepcopy(next(c for c in n["cases"]if c["name"]==name));m=b["result"]
 if kind=="lost_partition":m["faces"].pop(0)
 elif kind=="wrong_winner":next(f for f in m["faces"]if f["holes"])["attrs"]={"vendor":9}
 elif kind=="lost_empty_fallback":next(f for f in m["faces"]if f["holes"])["attrs"]={}
 elif kind=="lost_hole":next(f for f in m["faces"]if f["holes"])["holes"]=[]
 elif kind=="lost_wire":m["edges"]=[e for e in m["edges"]if e["incidence"]]
 elif kind=="lost_wire_flags":next(e for e in m["edges"]if e["curve"]==81)["soft"]=False
 elif kind=="changed_vertex_owner":m["vertex_origins"][0]=1000
 elif kind=="wrong_winding":m["faces"][0]["vertices"].reverse()
 else:b["error"]=None
 try:validate(a,b)
 except AssertionError:neg.append(kind)
 else:raise AssertionError("accepted corruption: "+kind)
report={"passed":True,"cases":rows,"negatives":neg,"nested":n["nested"],"scope":"Five exact source planar face/attribute, vertex survivor and edge payload cases; one source-accepted invalid-loop atomic safeguard. Source cached duplicate face references are observed separately; native incidence stays unique. Actual app dispatch/VCB/Undo/Redo/Finish, no OS gesture claim."}
Path(sys.argv[3]).write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
