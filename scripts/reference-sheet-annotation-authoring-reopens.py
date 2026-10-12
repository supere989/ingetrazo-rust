#!/usr/bin/env python3
"""Read native authoring saves with the unchanged pinned original IGZ reader."""
import argparse,json,os,subprocess,sys
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);a=p.parse_args();source=a.source.resolve();native=a.native.resolve();os.environ.setdefault('QT_QPA_PLATFORM','offscreen');original=Path(__file__).resolve().parents[1]/'ingetrazo';assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9';sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication
from core.scene import Scene
from formats import igz
app=QApplication([]);os.chdir(source);s=json.loads((source/'original-annotations.json').read_text());n=json.loads((native/'native-annotations.json').read_text());assert [r['name']for r in n['cases']]==[r['name']for r in s['cases']];rows=[]
for c in n['cases']:
 path=(native/c['saved']).resolve();assert path.is_relative_to(native)and path.is_file();scene=Scene();igz.load_into(scene,path);assert len(scene.compositions)==1;rows.append(dict(case=c['name'],composition=scene.compositions[0].to_dict()))
(native/'native-reopens.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n')
deleted=[];failures=[];full=native/'native-delete-reopens.json';full.unlink(missing_ok=True)
for c in n['cases']:
 if c['name']=='cancel-degenerate':continue
 path=(native/(c['name']+'-delete-control.igz')).resolve()
 try:
  assert path.is_relative_to(native)and path.is_file(),str(path);scene=Scene();igz.load_into(scene,path);assert len(scene.compositions)==1;deleted.append(dict(case=c['name'],composition=scene.compositions[0].to_dict()))
 except Exception as error:failures.append(dict(case=c['name'],path=str(path),error=repr(error)))
(native/'native-delete-reopens.partial.json').write_text(json.dumps(dict(rows=deleted,failures=failures),ensure_ascii=False,indent=2)+'\n')
assert not failures,('Original deletion readers refused; completed main and partial deletion observations retained',failures)
full.write_text(json.dumps(deleted,ensure_ascii=False,indent=2)+'\n');print(json.dumps(dict(reader_pairs=len(rows),delete_reader_pairs=len(deleted),source_pin='6be29fe437a7cd992d4e079f71720821b25216b9')))
