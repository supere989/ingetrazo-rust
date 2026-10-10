#!/usr/bin/env python3
"""Strict literal sample comparison, mandatory inputs and negative controls."""
import json,sys,copy
from pathlib import Path
required={"texture-quadrants","texture-repeat","texture-alpha","texture-cutout","texture-back"}
source=json.loads((Path(sys.argv[1])/"original-alpha-results.json").read_text());native=json.loads((Path(sys.argv[2])/"native-alpha-results.json").read_text())
def check(actual):
 rows={r["case"]:r for r in actual["cases"]};refs={r["case"]:r for r in source["cases"]}
 assert set(rows)==required and set(refs)==required,"Mandatory five-case corpus must be complete"
 for name in sorted(required):
  a,b=rows[name]["samples"],refs[name]["samples"]
  assert a==b,f"{name}: {a} != {b}"

check(native)
for corrupt in ["pixel","missing"]:
 bad=copy.deepcopy(native)
 if corrupt=="pixel":bad["cases"][0]["samples"][0][0]^=1
 else:bad["cases"].pop()
 try:check(bad)
 except AssertionError:pass
 else:raise AssertionError("Negative control incorrectly passed: "+corrupt)
print("PASS five actual original/native texture cases,25 exact RGBA samples,two negative controls")
