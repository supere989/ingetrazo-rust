#!/usr/bin/env python3
"""Read six native saved files with the untouched pinned original IGZ reader."""
import argparse,json,sys,subprocess
from pathlib import Path
PIN="6be29fe437a7cd992d4e079f71720821b25216b9"
p=argparse.ArgumentParser();p.add_argument("source",type=Path);p.add_argument("native",type=Path);p.add_argument("--original",type=Path,default=Path(__file__).resolve().parents[1]/"ingetrazo");a=p.parse_args()
assert subprocess.check_output(["git","-C",str(a.original),"rev-parse","HEAD"],text=True).strip()==PIN
sys.path.insert(0,str(a.original))
from core.scene import Scene
from formats import igz
source=json.loads((a.source/"original-clipboard.json").read_text());native=json.loads((a.native/"native-clipboard.json").read_text());assert native["source_pin"]==source["source_pin"]==PIN
assert [c["name"] for c in native["cases"]]==[c["name"] for c in source["cases"]]
rows=[];root=a.native.resolve()
for case in native["cases"]:
 path=(root/case["saved"]).resolve();assert path.is_relative_to(root) and path.is_file(),path
 scene=Scene();igz.load_into(scene,path)
 rows.append({"case":case["name"],"compositions":[c.to_dict() for c in scene.compositions]})
(root/"native-reopens.json").write_text(json.dumps(rows,ensure_ascii=False,allow_nan=False,indent=2)+"\n")
print(json.dumps({"original_reader_pairs":len(rows),"source_pin":PIN}))
