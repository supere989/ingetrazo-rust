#!/usr/bin/env python3
"""Actual original IGZ reader over the three native image-authoring outputs."""
import os,sys,json,subprocess
from pathlib import Path
source=Path(sys.argv[1]).resolve();native=Path(sys.argv[2]).resolve();repo=Path(__file__).resolve().parents[1]
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
for key,name in [("XDG_CONFIG_HOME","reader-config"),("XDG_DATA_HOME","reader-data"),("XDG_CACHE_HOME","reader-cache")]:os.environ[key]=str(native/name)
sys.path.insert(0,str(repo/"ingetrazo"))
pin="6be29fe437a7cd992d4e079f71720821b25216b9"
assert subprocess.check_output(["git","-C",str(repo/"ingetrazo"),"rev-parse","HEAD"],text=True).strip()==pin
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
app=QApplication([]);os.chdir(source)
data=json.loads((source/"original-picture-authoring.json").read_text());rows=[]
assert [c["name"]for c in data["cases"]]==["authoring","missing-fallback","aspect-selection"]
for c in data["cases"]:
 s=Scene();igz.load_into(s,native/(c["name"]+"-native.igz"))
 assert len(s.compositions)==1
 comp=s.compositions[0]
 rows.append(dict(case=c["name"],composition=comp.to_dict(),paths=[v.path for v in comp.images]))
(native/"native-reopens.json").write_text(json.dumps(rows,ensure_ascii=False,indent=2)+"\n")
print(json.dumps(dict(reader_pairs=len(rows))))
