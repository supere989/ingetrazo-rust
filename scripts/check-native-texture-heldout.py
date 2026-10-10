#!/usr/bin/env python3
"""Mandatory heldout texture samples; failures remain failures."""
import json,sys,copy
from pathlib import Path
required={"tex-back-color-front","tex-back-color-reversed","tex-trans-back-color-front","tex-trans-back-color-reversed","tex-back-opaque-color-reversed","color-back-tex-reversed","color-back-tex-front","tex-hole","tex-group-reflected","tex-group-slanted","tex-mixed-order-tex-first","tex-mixed-order-color-first","tex-mixed-backcolor-phase","tex-shaded-average","tex-xray"}
source=json.loads((Path(sys.argv[1])/"original-alpha-results.json").read_text());native=json.loads((Path(sys.argv[2])/"native-alpha-results.json").read_text())
refs={r["case"]:r for r in source["cases"]}
def compare(data):
 rows={r["case"]:r for r in data["cases"]}
 assert set(rows)==required and set(refs)==required,"Missing or unexpected literal heldout case"
 return {name:rows[name]["samples"]==refs[name]["samples"] for name in sorted(required)}
result=compare(native)
bad=copy.deepcopy(native);row=next(r for r in bad["cases"] if r["case"]=="tex-hole");row["samples"][0][0]^=1
assert not compare(bad)["tex-hole"],"Pixel mutation incorrectly accepted"
bad=copy.deepcopy(native);bad["cases"].pop()
try:compare(bad)
except AssertionError:pass
else:raise AssertionError("Missing input incorrectly accepted")
print(json.dumps({"cases":result,"passing":sum(result.values()),"failed":[n for n,v in result.items() if not v],"negative_controls":2},indent=2))
sys.exit(0 if all(result.values()) else 1)
