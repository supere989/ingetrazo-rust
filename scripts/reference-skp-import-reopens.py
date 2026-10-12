#!/usr/bin/env python3
"""Actual original IGZ reader for both immutable source and native saved files."""
import os,sys,json
from pathlib import Path
source=Path(sys.argv[1]).resolve();native=Path(sys.argv[2]).resolve()
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,'/work/ingetrazo')
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
