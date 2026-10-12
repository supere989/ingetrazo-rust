#!/usr/bin/env python3
"""Original saved-file readers for specialized composer workflows; no source recapture."""
import argparse,json,os,sys,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);a=p.parse_args();source=a.source.resolve();native=a.native.resolve();repo=Path(__file__).resolve().parents[1];original=repo/'ingetrazo';assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original));os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
app=QApplication([]);os.chdir(source);s=json.loads((source/'original-specialized.json').read_text());n=json.loads((native/'native-specialized.json').read_text());assert [c['name']for c in s['cases']]==[c['name']for c in n['cases']];rows=[];failures=[]
for case in n['cases']:
 try:
  path=(native/case['saved']).resolve();path.relative_to(native);assert path.is_file();scene=Scene();igz.load_into(scene,path);rows.append(dict(case=case['name'],compositions=[c.to_dict()for c in scene.compositions]))
 except Exception as error:failures.append(dict(case=case['name'],error=repr(error)))
(native/'native-reopens.partial.json').write_text(json.dumps(dict(rows=rows,failures=failures),indent=2)+'\n');assert not failures,failures;(native/'native-reopens.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n');print(json.dumps(dict(original_reader_pairs=len(rows))))
