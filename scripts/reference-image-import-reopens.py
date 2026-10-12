#!/usr/bin/env python3
"""Actual original IGZ reader, preserving resource content and model ownership."""
import ast,hashlib,json,os,subprocess,sys
from pathlib import Path
D='image-import';source=Path(sys.argv[1]).resolve();native=Path(sys.argv[2]).resolve()
repo=Path(__file__).resolve().parents[1];original=(repo/'ingetrazo').resolve()
assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QImage,QMatrix4x4 as M
from core.scene import Scene
from formats import igz
app=QApplication([])
m=ast.parse((repo/'scripts/reference-nested-import.py').read_text())
exec(compile(ast.Module(body=[v for v in m.body if isinstance(v,ast.FunctionDef)and v.name in ['vec','mesh_state','shape']],type_ignores=[]),'existing-model-observer','exec'))
def observe(path):
 s=Scene();igz.load_into(s,path);row=shape(s);row['datum']=None if s.georef is None else s.georef.to_dict();row['image_planes']=[]
 for v in s.image_planes:
  data=v.to_dict();p=Path(v.path);q=QImage(str(p));assert not q.isNull()
  q=q.convertToFormat(QImage.Format_RGBA8888);raw=bytes(q.constBits());pixels=b''.join(raw[y*q.bytesPerLine():y*q.bytesPerLine()+q.width()*4]for y in range(q.height()))
  data['texture']['path']={'encoded_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'rgba_sha256':hashlib.sha256(pixels).hexdigest(),'size':[q.width(),q.height()]};row['image_planes'].append(data)
 return row
rows=[]
for c in json.loads((source/f'original-{D}.json').read_text())['cases']:
 if not c.get('saved'):continue
 a=(source/c['saved']).resolve();b=(native/c['saved']).resolve();assert a.is_relative_to(source)and b.is_relative_to(native)
 rows.append({'case':c['name'],'original':observe(a),'native':observe(b)})
(native/f'reopens-{D}.json').write_text(json.dumps(rows,indent=2,allow_nan=False)+'\n');print(json.dumps({'actual_reader_pairs':len(rows)}))
