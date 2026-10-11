#!/usr/bin/env python3
"""Original Qt shaded opaque/translucent witnesses, strict literal samples."""
import copy,json,sys
from pathlib import Path
required={"shaded-opaque-back","shaded-opaque","shaded-trans-front","shaded-trans-front-cull","shaded-trans-back","shaded-mixed-asset"}
source=json.loads((Path(sys.argv[1])/"original-alpha-results.json").read_text())
native=json.loads((Path(sys.argv[2])/"native-alpha-results.json").read_text())
refs={r["case"]:r for r in source["cases"]}
def check(value):
 rows={r["case"]:r for r in value["cases"]}
 assert set(rows)==set(refs)==required and len(value["cases"])==len(source["cases"])==6
 return {name:rows[name]["samples"]==refs[name]["samples"] for name in sorted(required)}
results=check(native)
for name in sorted(required):
 bad=copy.deepcopy(native);row=next(r for r in bad["cases"] if r["case"]==name);row["samples"][0][0]^=1
 assert not check(bad)[name],name
print(json.dumps({"cases":results,"exact_sample_vectors":sum(results.values())*5,"pixel_mutation_controls":6,"scope":"Actual source shaded opacity front/back and mixed same-asset means/sampling. Samples only; full render parity remains separately checked."},indent=2))
raise SystemExit(0 if all(results.values()) else 1)
