#!/usr/bin/env python3
"""Actual original reader observations for frozen native frame-sync saves."""
import argparse,json,os,sys,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);a=p.parse_args();source=a.source.resolve();native=a.native.resolve();os.environ.setdefault('QT_QPA_PLATFORM','offscreen');original=Path(__file__).resolve().parents[1]/'ingetrazo';assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
from frame_annotation_observations import geometry,annotations
app=QApplication([]);os.chdir(source);s=json.loads((source/'original-frame-annotations.json').read_text());n=json.loads((native/'native-frame-annotations.json').read_text());assert [c['name']for c in s['cases']]==[c['name']for c in n['cases']];rows=[];failures=[]
for case in n['cases']:
 try:
  path=(native/case['saved']).resolve();assert path.is_relative_to(native)and path.is_file();scene=Scene();igz.load_into(scene,path);assert len(scene.compositions)==1;rows.append(dict(case=case['name'],composition=scene.compositions[0].to_dict(),model_geometry=geometry(scene),model_annotations=annotations(scene)))
 except Exception as error:failures.append(dict(case=case['name'],error=repr(error)))
(native/'native-reopens.partial.json').write_text(json.dumps(dict(rows=rows,failures=failures),ensure_ascii=False,indent=2)+'\n');assert not failures,failures;(native/'native-reopens.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n');print(json.dumps(dict(original_reader_pairs=len(rows))))
