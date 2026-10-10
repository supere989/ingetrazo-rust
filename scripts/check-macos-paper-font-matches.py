#!/usr/bin/env python3
"""Strict selected physical face comparison; no raster equivalence claim."""
import sys,json
from pathlib import Path
original=json.loads(Path(sys.argv[1]).read_text());native=json.loads(Path(sys.argv[2]).read_text())
names=["helvetica","helvetica-bold","helvetica-italic","times-bold-italic","cjk","arabic"]
assert original["source_pin"]=="6be29fe437a7cd992d4e079f71720821b25216b9" and original["qt"]=="6.12.0"
assert [r["name"]for r in original["cases"]]==names and len(native)==6
rows=[]
for source,got in zip(original["cases"],native):
 errors=[]
 for key in ["family","bold","italic","text"]:
  if source[key]!=got.get(key):errors.append("request mismatch: "+key)
 if "error" in got:errors.append("native refusal: "+str(got["error"]))
 else:
  if got.get("coverage") is not True:errors.append("missing character coverage")
  if got.get("warm_same_owner") is not True:errors.append("missing stable warm ownership")
  if got.get("chosen_bold")!=got.get("bold") or got.get("chosen_italic")!=got.get("italic"):errors.append("requested style mismatch")
 observed=source["observations"]
 runs=[run for call in observed for run in call["supplementary_qtextlayout_runs"]]
 if not observed or not runs:errors.append("missing source glyph runs")
 for run in runs:
  if not any(face["path"]==got.get("path") and face["index"]==got.get("index") and face["postscript"]==got.get("postscript") for face in run["physical_faces"]):errors.append("physical face mismatch")
 rows.append({"name":source["name"],"passed":not errors,"errors":errors})
passed=all(row["passed"] for row in rows)
print(json.dumps({"passed":passed,"matched":[r["name"] for r in rows if r["passed"]],"cases":rows,"scope":"Exact Qt supplementary glyph-run physical face/native single-face match; actual Canvas pixels still pending"},indent=2))
if not passed:raise SystemExit(1)
