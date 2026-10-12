#!/usr/bin/env python3
"""Actual pinned original reader for finite native gesture saves."""
import argparse,json,os,subprocess,sys
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);a=p.parse_args();source=a.source.resolve();native=a.native.resolve();os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
original=Path(__file__).resolve().parents[1]/'ingetrazo';assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
app=QApplication([]);os.chdir(source)
expected=json.loads((source/'original-gestures.json').read_text());actual=json.loads((native/'native-gestures.json').read_text());names=[r['name']for r in expected['cases']];assert [r['name']for r in actual['cases']]==names
rows=[]
for c in actual['cases']:
 path=(native/c['saved']).resolve();assert path.is_relative_to(native)and path.is_file();scene=Scene();igz.load_into(scene,path);assert len(scene.compositions)==1;rows.append(dict(case=c['name'],composition=scene.compositions[0].to_dict()))
(native/'native-reopens.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps(dict(reopens=len(rows),source_pin='6be29fe437a7cd992d4e079f71720821b25216b9')))
