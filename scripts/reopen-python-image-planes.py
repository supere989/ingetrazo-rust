#!/usr/bin/env python3
import os,sys,json,hashlib
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from core.scene import Scene
from formats import igz
source=json.loads(Path(sys.argv[1]).read_text())["cases"];folder=Path(sys.argv[2]);rows={}
for key,states in source.items():
 s=Scene();igz.load_into(s,folder/(key+".igz"));planes=[]
 for p in s.image_planes:
  assert Path(p.path).exists(),"reopened embedded scan missing";assert hashlib.sha256(Path(p.path).read_bytes()).digest()==hashlib.sha256(Path(sys.argv[3]).read_bytes()).digest(),"scan bytes changed"
  value=p.to_dict();value["texture"]["path"]="scan.png";planes.append(value)
 assert planes==states[-1]["planes"],key+" reopen mismatch"
 rows[key]=planes
print(json.dumps({"passed":True,"cases":rows,"embedded_assets":"every reopened scan SHA256 equals literal original bytes"},indent=2))
