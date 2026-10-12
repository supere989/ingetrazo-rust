#!/usr/bin/env python3
"""Reopen each actual source/native DAE with the unchanged pinned original reader."""
import hashlib,json,os,subprocess,sys,xml.etree.ElementTree as ET
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
source=Path(sys.argv[1]).resolve();native=Path(sys.argv[2]).resolve();repo=Path(__file__).resolve().parents[1];original=(repo/'ingetrazo').resolve()
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
report=json.loads((source/'original-dae-export.json').read_text());assert report['source_pin']==PIN
assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()==PIN
sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QImage
from core.scene import Scene
from formats import dae,igz
import exchange_import_observations as observations
app=QApplication([]);NS='{http://www.collada.org/2005/11/COLLADASchema}'
def sha(data):return hashlib.sha256(data).hexdigest()
def payload(path):
 if not path.is_file():return {'encoded_sha256':None,'missing':True}
 image=QImage(str(path));assert not image.isNull(),'actual Qt cannot decode exported sidecar'
 image=image.convertToFormat(QImage.Format_RGBA8888);raw=bytes(image.constBits());raw=b''.join(raw[y*image.bytesPerLine():y*image.bytesPerLine()+image.width()*4]for y in range(image.height()))
 return {'encoded_sha256':sha(path.read_bytes()),'missing':False,'pixels':[image.width(),image.height()],'rgba_sha256':sha(raw)}
def texture(value):
 value=dict(value)
 if 'path' in value:value['resource']=payload(Path(value.pop('path')))
 return value
# Replace only the retained observer's resource-locator projection, never the
# original loader/model. UV/material metadata remains in the observed dict.
observations.texture=texture

def observe(folder,filename):
 relative=Path(filename);assert not relative.is_absolute()and '..'not in relative.parts
 path=(folder/relative).resolve();assert path.is_relative_to(folder)
 scene=Scene();dae.load_dae(scene,path)
 sidecars=[]
 for image in ET.parse(path).getroot().iter(NS+'image'):
  name=image.findtext(NS+'init_from');assert name
  relative=Path(name);assert not relative.is_absolute()and '..'not in relative.parts
  asset=(path.parent/relative).resolve();assert asset.is_relative_to(path.parent)
  sidecars.append({'file':name,'resource':payload(asset)})
 return {'scene':observations.scene_state(scene),'sidecars':sidecars}
def saved(folder,filename):
 relative=Path(filename);assert not relative.is_absolute()and '..'not in relative.parts
 path=(folder/relative).resolve();assert path.is_relative_to(folder)
 scene=Scene();igz.load_into(scene,path);return observations.scene_state(scene)
rows=[]
for case in report['cases']:
 for export in case['exports']:
  row={'case':case['name'],'phase':export['phase'],'original':observe(source,export['file']),'native':observe(native,export['file']),'saved_original':saved(source,case['saved']),'saved_native':saved(native,case['saved'])};rows.append(row)
  (native/'native-dae-reopens.partial.json').write_text(json.dumps(rows,indent=2,allow_nan=False)+'\n')
assert rows,'no actual exported files to reopen'
(native/'native-dae-reopens.json').write_text(json.dumps(rows,indent=2,allow_nan=False)+'\n');print(json.dumps({'actual_original_readers':len(rows),'source_pin':PIN}))
