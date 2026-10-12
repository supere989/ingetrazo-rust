#!/usr/bin/env python3
"""Actual original reader for missing OBJ/DXF App saved workflows."""
import json,os,subprocess,sys
from pathlib import Path
repo=Path(__file__).resolve().parents[1];original=(repo/'ingetrazo').resolve();source=Path(sys.argv[1]).resolve();native=Path(sys.argv[2]).resolve()
pin='6be29fe437a7cd992d4e079f71720821b25216b9';assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()==pin
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
from exchange_import_observations import scene_state
app=QApplication([])
def load(base,name):
 p=(base/name).resolve();assert p.is_relative_to(base);s=Scene();igz.load_into(s,p);return scene_state(s)
rows=[]
for c in json.loads((source/'original-unit-import.json').read_text())['cases']:
 if c['saved'] is not None:rows.append({'case':c['name'],'original':load(source,c['saved']),'native':load(native,c['saved'])})
(native/'reopens-unit-import.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps({'actual_reader_pairs':len(rows)}))
