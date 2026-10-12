#!/usr/bin/env python3
"""Actual pinned original reader of source and native default figure IGZs."""
import argparse,copy,hashlib,json,os,subprocess,sys
from pathlib import Path
ap=argparse.ArgumentParser();ap.add_argument('source',type=Path);ap.add_argument('native',type=Path);a=ap.parse_args();source=a.source.resolve();native=a.native.resolve()
repo=Path(__file__).resolve().parents[1];original=(repo/'ingetrazo').resolve();report=json.loads((source/'original-default-figure.json').read_text())
assert report['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()==report['source_pin']
sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QImage
from core.scene import Scene
from formats import igz
app=QApplication([]);resources={};out=native/'reader-resources';out.mkdir(exist_ok=True);(out/'assets').mkdir(exist_ok=True)
def sha(data):return hashlib.sha256(data).hexdigest()
def resource(path):
 path=Path(path);encoded=path.read_bytes();image=QImage(str(path));assert not image.isNull();image=image.convertToFormat(QImage.Format_RGBA8888)
 packed=bytes(image.constBits());rgba=b''.join(packed[y*image.bytesPerLine():y*image.bytesPerLine()+image.width()*4]for y in range(image.height()))
 digest=sha(encoded);name=digest+'.png';raw=digest+'.rgba'
 if digest not in resources:
  (out/'assets'/name).write_bytes(encoded);(out/'assets'/raw).write_bytes(rgba);resources[digest]={'file':'assets/'+name,'rgba':'assets/'+raw,'encoded_sha256':digest,'rgba_sha256':sha(rgba),'size':[image.width(),image.height()]}
 return {'encoded_sha256':digest,'rgba_sha256':sha(rgba),'size':[image.width(),image.height()]}
def observe(scene):
 groups=[]
 for g in scene.groups:
  faces=[]
  for face in g.mesh.faces:
   attrs=copy.deepcopy(face.attrs)
   if 'texture'in attrs and'path'in attrs['texture']:attrs['texture']['path']=resource(attrs['texture']['path'])
   faces.append({'outer':[[p.x(),p.y(),p.z()]for p in face.vertices],'holes':[[[p.x(),p.y(),p.z()]for p in h]for h in face.holes],'attrs':attrs})
  groups.append({'name':g.name,'component':g.component,'is_component':g.is_component(),'billboard':g.billboard,'hidden':g.hidden,'layer':g.layer,'children':len(g.children),'faces':faces})
 return {'groups':groups,'loose_faces':len(scene.mesh.faces),'images':len(scene.image_planes)}
rows=[]
for case in report['cases']:
 saved=case['saved'];assert Path(saved).name==saved
 pair={'name':case['name']}
 for label,root in [('original',source),('native',native)]:
  scene=Scene();igz.load_into(scene,root/saved);pair[label]=observe(scene)
 rows.append(pair)
(native/'reopens-default-figure.json').write_text(json.dumps({'cases':rows},indent=2,allow_nan=False)+'\n')
