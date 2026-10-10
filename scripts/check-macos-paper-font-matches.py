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
 for key in ["family","bold","italic","text"]:assert source[key]==got[key],(source["name"],key)
 assert got["coverage"] and got["warm_same_owner"]
 assert got["chosen_bold"]==got["bold"] and got["chosen_italic"]==got["italic"]
 observed=source["observations"];assert observed
 runs=[run for call in observed for run in call["supplementary_qtextlayout_runs"]];assert runs
 for run in runs:
  assert any(Path(face["path"])==Path(got["path"]) and face["index"]==got["index"] and face["postscript"]==got["postscript"] for face in run["physical_faces"]),(source["name"],run,got)
 rows.append(source["name"])
print(json.dumps({"matched":rows,"scope":"Exact Qt supplementary glyph-run physical face/native single-face match; actual Canvas pixels still pending"}))
