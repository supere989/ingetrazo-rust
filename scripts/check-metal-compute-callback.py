#!/usr/bin/env python3
"""Strict actual native GPU whole-image proof; refuses missing/unsupported results."""
import hashlib,json,pathlib,sys
root=pathlib.Path(sys.argv[1]);repo=pathlib.Path(__file__).resolve().parents[1]
result=json.loads((root/"result.json").read_text())
for key in ["passed","read_write_tier2","source_order_negative","phase_negative","partial_plan_budget_rollback","missing_ui_negative"]:
    if result.get(key)is not True:raise ValueError("missing successful control:"+key)
if result.get("actual_qt_whole_images")!=7 or result.get("final_ledger")!=0 or result.get("presentation_claim")is not False:raise ValueError("incorrect proof scope")
rows=[]
for name in ["alpha-0","alpha-1","alpha-127","alpha-128","alpha-254","alpha-255","mixed-ui-cad"]:
    source=repo/"docs/audits"/("metal-compute-callback/source" if name=="mixed-ui-cad" else "metal-cad2d-offscreen/original")/(name+".rgba")
    actual=(root/(name+".rgba")).read_bytes();expected=source.read_bytes()
    if len(actual)!=16384 or actual!=expected:raise ValueError("strict complete Qt image mismatch:"+name)
    rows.append({"name":name,"sha256":hashlib.sha256(actual).hexdigest()})
for name,reference in [("reversed-order","alpha-128"),("missing-final-ui","mixed-ui-cad")]:
    a=(root/(name+".rgba")).read_bytes();b=(root/(reference+".rgba")).read_bytes()
    if len(a)!=16384 or a==b:raise ValueError("actual GPU negative failed:"+name)
print(json.dumps({"passed":True,"actual_gpu_images":rows,"actual_gpu_negatives":2,"presentation_claim":False},indent=2))
