#!/usr/bin/env python3
"""Actual original IGZ reader for both immutable source and native saved files."""
import os,sys,json,subprocess
from pathlib import Path
source=Path(sys.argv[1]).resolve();native=Path(sys.argv[2]).resolve()
repo=Path(__file__).resolve().parents[1];original_root=(repo/'ingetrazo').resolve()
original=json.loads((source/'original-skp.json').read_text())
assert subprocess.check_output(['git','-C',str(original_root),'rev-parse','HEAD'],text=True).strip()==original['source_pin']
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(original_root))
from PySide6.QtWidgets import QApplication
from formats import igz
from core.scene import Scene
from skp_import_observations import scene_state
app=QApplication([]);original=json.loads((source/'original-skp.json').read_text());rows=[]
def load(path):
 s=Scene();igz.load_into(s,path);return s
for case in original['cases']:
 if case['saved'] is not None:
  rows.append({'case':case['name'],'original':scene_state(load(source/case['saved'])),'native':scene_state(load(native/case['saved']))})
(native/'reopens-skp.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps({'original_readers':len(rows)}))
