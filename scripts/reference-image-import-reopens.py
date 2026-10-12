#!/usr/bin/env python3
"""Actual original IGZ reader, preserving resource content and model ownership."""
import ast,hashlib,json,os,subprocess,sys,zipfile
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
def digest_resource(p):
 q=QImage(str(p));assert not q.isNull(),str(p);q=q.convertToFormat(QImage.Format_RGBA8888);raw=bytes(q.constBits());pixels=b''.join(raw[y*q.bytesPerLine():y*q.bytesPerLine()+q.width()*4]for y in range(q.height()))
 return {'encoded_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'rgba_sha256':hashlib.sha256(pixels).hexdigest(),'size':[q.width(),q.height()]}
def normalize_materials(value):
 if isinstance(value,dict):
  tex=value.get('texture')
  if isinstance(tex,dict)and isinstance(tex.get('path'),str):tex['path']=digest_resource(Path(tex['path']))
  for child in value.values():normalize_materials(child)
 elif isinstance(value,list):
  for child in value:normalize_materials(child)
def observe(path):
 s=Scene();igz.load_into(s,path);row=shape(s);row['datum']=None if s.georef is None else s.georef.to_dict();row['image_planes']=[]
 for v in s.image_planes:
  data=v.to_dict();p=Path(v.path);q=QImage(str(p));assert not q.isNull()
  q=q.convertToFormat(QImage.Format_RGBA8888);raw=bytes(q.constBits());pixels=b''.join(raw[y*q.bytesPerLine():y*q.bytesPerLine()+q.width()*4]for y in range(q.height()))
  data['texture']['path']={'encoded_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'rgba_sha256':hashlib.sha256(pixels).hexdigest(),'size':[q.width(),q.height()]};row['image_planes'].append(data)
 normalize_materials(row)
 return row
rows=[]
for c in json.loads((source/f'original-{D}.json').read_text())['cases']:
 if not c.get('saved'):continue
 a=(source/c['saved']).resolve();b=(native/c['saved']).resolve();assert a.is_relative_to(source)and b.is_relative_to(native)
 rows.append({'case':c['name'],'original':observe(a),'native':observe(b)})
# Bind frozen source locators to immutable original bytes and source input embeds.
a=json.loads((source/f'original-{D}.json').read_text());mapping={}
def collect(value):
 if isinstance(value,dict):
  tex=value.get('texture')
  if isinstance(tex,dict)and isinstance(tex.get('path'),str):
   raw=tex['path'];prefix='/work/ingetrazo/';assert raw.startswith(prefix),('unbound source material',raw)
   relative=raw[len(prefix):];p=(original/relative).resolve();assert p.is_relative_to(original)
   expected=subprocess.check_output(['git','-C',str(original),'show','6be29fe437a7cd992d4e079f71720821b25216b9:'+relative]);assert p.read_bytes()==expected
   identity=digest_resource(p);q=QImage(str(p)).convertToFormat(QImage.Format_RGBA8888);bits=bytes(q.constBits());rgba=b''.join(bits[y*q.bytesPerLine():y*q.bytesPerLine()+q.width()*4]for y in range(q.height()));rgba_name='source-materials/'+identity['encoded_sha256']+'.rgba';dest=native/rgba_name;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(rgba)
   assert hashlib.sha256(rgba).hexdigest()==identity['rgba_sha256']
   mapping[raw]={'original_path':relative,'original_commit':'6be29fe437a7cd992d4e079f71720821b25216b9','resource':identity,'decoded_rgba':rgba_name,'input_members':[]}
  for child in value.values():collect(child)
 elif isinstance(value,list):
  for child in value:collect(child)
for state in a['states']:collect(state['scene']['geometry'])
for raw,row in mapping.items():
 for c in a['cases']:
  used=any(raw in json.dumps(v['scene']['geometry'])for v in a['states']if v['case']==c['name'])
  if not used:continue
  with zipfile.ZipFile(source/c['input'])as z:
   found=[name for name in z.namelist()if name.startswith('textures/')and hashlib.sha256(z.read(name)).hexdigest()==row['resource']['encoded_sha256']]
   assert len(found)==1,('source embedded material bytes',c['name'],raw)
   row['input_members'].append({'case':c['name'],'input':c['input'],'member':found[0],'sha256':row['resource']['encoded_sha256']})
(native/'source-material-observations.json').write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','materials':mapping},indent=2,allow_nan=False)+'\n')
(native/f'reopens-{D}.json').write_text(json.dumps(rows,indent=2,allow_nan=False)+'\n');print(json.dumps({'actual_reader_pairs':len(rows)}))
