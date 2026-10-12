#!/usr/bin/env python3
"""Missing-only actual original MainWindow scale figure and resource ownership."""
import argparse,copy,hashlib,json,os,subprocess,sys,traceback
from pathlib import Path
ap=argparse.ArgumentParser();ap.add_argument('output',type=Path);a=ap.parse_args();out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
for key,name in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[key]=str(out/name)
repo=Path(__file__).resolve().parents[1];original=(repo/'ingetrazo').resolve();c=json.loads((repo/'docs/audits/ai-bridge-python/default-figure-contract.json').read_text())
assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()==c['source_pin']
for row in c['original_sources']:assert hashlib.sha256((original/row['path']).read_bytes()).hexdigest()==row['sha256']
sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QSettings
from PySide6.QtGui import QImage
from views.main_window import MainWindow
from core.scene import Scene
from formats import igz
app=QApplication([]);errors=[];sys.excepthook=lambda typ,e,tb:errors.append({'type':typ.__name__,'error':str(e),'traceback':''.join(traceback.format_exception(typ,e,tb))})
(out/'assets').mkdir();resources={};hosts=[];report={'source_pin':c['source_pin'],'cases':[],'resources':resources,'callback_errors':errors}
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
  groups.append({'name':g.name,'component':g.component,'is_component':g.is_component,'billboard':g.billboard,'hidden':g.hidden,'layer':g.layer,'children':len(g.children),'faces':faces})
 return {'groups':groups,'loose_faces':len(scene.mesh.faces),'images':len(scene.image_planes)}
try:
 for recipe in c['cases']:
  settings=QSettings()
  if recipe['setting']is None:settings.remove('new_document/scale_figure')
  else:settings.setValue('new_document/scale_figure',recipe['setting'])
  settings.sync();host=MainWindow();hosts.append(host);app.processEvents();vp=host.viewport
  state=observe(vp.scene);expected=0 if recipe['setting']=='0' else 1
  assert len(state['groups'])==expected and len(vp.history.undo_stack)==len(vp.history.redo_stack)==0 and host._saved_version==vp.scene.version
  if expected:assert len(state['groups'][0]['faces'])==1 and state['groups'][0]['billboard']is True
  saved=recipe['name']+'-saved.igz';igz.save_scene(vp.scene,out/saved);loaded=Scene();igz.load_into(loaded,out/saved);reopen=observe(loaded);assert reopen==state
  report['cases'].append({'name':recipe['name'],'setting':recipe['setting'],'state':state,'clean':host._saved_version==vp.scene.version,'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack),'saved':saved,'reopen':reopen,'identity_diagnostic':[g.uid for g in vp.scene.groups]})
  (out/'original-default-figure.partial.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n');host.hide()
 assert errors==[],errors
 (out/'original-default-figure.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n');print(json.dumps({'cases':len(report['cases']),'resources':len(resources),'callback_errors':errors}))
finally:
 for host in hosts:host.hide()
