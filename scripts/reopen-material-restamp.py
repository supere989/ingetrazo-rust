#!/usr/bin/env python3
"""Read native App exports with the immutable original loader."""
import json,sys,subprocess,os
from pathlib import Path
from importlib.machinery import SourceFileLoader
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
SOURCE=Path(__file__).resolve().parents[1]/"ingetrazo"
sys.path.insert(0,str(SOURCE))
from core.scene import Scene
from formats.igz import load_into
reference=SourceFileLoader("material_reference",str(Path(__file__).with_name("reference-material-restamp.py"))).load_module()
assert subprocess.check_output(["git","-C",str(SOURCE),"rev-parse","HEAD"],text=True).strip()==reference.PIN
out=[]
for name in ["root","classic","placed","nested","tray"]:
 scene=Scene();path=Path(sys.argv[1])/(name+".igz");load_into(scene,path)
 raw=json.loads(path.read_text())["scene"]
 if name!="tray":assert raw["faces"][0]["vendor"]=={"id":71},"opaque archive field lost"
 out.append({"case":name,"state":reference.state(scene,"reopened")})
Path(sys.argv[2]).write_text(json.dumps({"source_pin":reference.PIN,"cases":out},indent=2)+"\n")
