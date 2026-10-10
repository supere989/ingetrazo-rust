#!/usr/bin/env python3
"""Physical face/traits plus source-requested PDF synthesis policy; no Canvas bold claim."""
import sys,json
from pathlib import Path
NAMES=["helvetica","helvetica-bold","helvetica-italic","times-bold-italic","cjk","arabic"]
def compare(original,native):
 assert original["source_pin"]=="6be29fe437a7cd992d4e079f71720821b25216b9" and original["qt"]=="6.12.0"
 assert [r["name"]for r in original["cases"]]==NAMES and len(native)==6
 rows=[]
 for source,got in zip(original["cases"],native):
  errors=[]
  for key in ["family","bold","italic","text"]:
   if source[key]!=got.get(key):errors.append("request mismatch: "+key)
  if "error" in got:errors.append("native refusal: "+str(got["error"]))
  else:
   if got.get("coverage") is not True:errors.append("missing character coverage")
   if got.get("warm_same_owner") is not True:errors.append("missing stable warm ownership")
  observed=source["observations"];runs=[run for call in observed for run in call["supplementary_qtextlayout_runs"]]
  if not observed or not runs:errors.append("missing source glyph runs")
  for run in runs:
   matches=[face for face in run["physical_faces"]if face["path"]==got.get("path") and face["index"]==got.get("index") and face["postscript"]==got.get("postscript")]
   if len(matches)!=1:errors.append("physical face mismatch");continue
   traits=matches[0]["physical_traits"]
   for trait in ["bold","italic"]:
    if traits["os2_"+trait]!=traits["head_"+trait]:errors.append("ambiguous source physical trait: "+trait)
    physical=traits["os2_"+trait]
    if got.get("chosen_"+trait)!=physical:errors.append("source physical trait mismatch: "+trait)
    expected=bool(source[trait]and not physical)
    if got.get("pdf_synthetic_"+trait)!=expected:errors.append("missing/wrong PDF synthesis policy: "+trait)
  rows.append({"name":source["name"],"passed":not errors,"errors":errors})
 return {"passed":all(r["passed"]for r in rows),"matched":[r["name"]for r in rows if r["passed"]],"cases":rows,"scope":"Actual Qt physical face/traits and requested PDF synthesis policy; pixel/layout gates independent; Canvas synthesized-bold still unsupported"}
def main():
 result=compare(json.loads(Path(sys.argv[1]).read_text()),json.loads(Path(sys.argv[2]).read_text()));print(json.dumps(result,indent=2));return 0 if result['passed']else 1
if __name__=='__main__':sys.exit(main())
