#!/usr/bin/env python3
"""Finite actual pinned Qt DEM/ProfileDock HTTP and disk-cache source observation."""
import argparse, hashlib, io, json, os, subprocess, sys, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('initial',type=Path);args=parser.parse_args()
out=args.output.resolve();assert not out.exists();out.mkdir(parents=True)
for key,folder in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:os.environ[key]=str(out/folder)
os.environ['QT_QPA_PLATFORM']='offscreen'
repo=Path(__file__).resolve().parents[1];original=repo/'ingetrazo';pin='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()==pin
sys.path.insert(0,str(original))
import numpy as np
from PySide6.QtCore import Qt,QByteArray,QBuffer,QIODevice,qVersion
from PySide6.QtGui import QImage,QVector3D
from PySide6.QtWidgets import QApplication,QMainWindow,QPushButton
from georef.dem import DEMSampler,DEMSource,AWS_TERRAIN,decode_elevation,bilinear,pixel_in_tile
from georef.tiles import TileCache,num2deg,tiles_covering
from georef.datum import SceneDatum
from georef.geopath import GeoPath
from views.viewport import Viewport
from views.profile_panel import ProfileDock
from formats import igz
from views.filedialogs import file_dialogs
app=QApplication([]);app.setOrganizationName('IngeTrazoDEMSourceProof');app.setApplicationName('dem-profile')
lock=threading.Lock();routes={};requests=[];active=0;peak=0
class Handler(BaseHTTPRequestHandler):
 def log_message(self,*args):pass
 def do_GET(self):
  global active,peak
  with lock:
   requests.append(self.path);active+=1;peak=max(peak,active);response=routes.get(self.path)
  try:
   if response is None:self.send_error(404);return
   gate,data,code=response
   assert gate.wait(20),'Owned response gate expired'
   self.send_response(code);self.send_header('Content-Type','image/png');self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
  except (BrokenPipeError,ConnectionResetError):pass
  finally:
   with lock:active-=1
server=ThreadingHTTPServer(('127.0.0.1',0),Handler);thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start();base='http://127.0.0.1:'+str(server.server_port)
def wait(predicate,label):
 deadline=time.monotonic()+15
 while not predicate():
  app.processEvents();time.sleep(.002)
  if time.monotonic()>deadline:raise AssertionError('Timed out: '+label)
 app.processEvents()
def png(encoding,seed):
 y,x=np.indices((256,256));height=(-35+seed*8+x/8+y/16)
 n=np.rint((height+32768)*256).astype(np.int64)if encoding=='terrarium'else np.rint((height+10000)*10).astype(np.int64)
 rgba=np.empty((256,256,4),dtype=np.uint8);rgba[:,:,0]=(n>>16)&255;rgba[:,:,1]=(n>>8)&255;rgba[:,:,2]=n&255;rgba[:,:,3]=255
 im=QImage(rgba.data,256,256,1024,QImage.Format_RGBA8888).copy();buf=QBuffer();buf.open(QIODevice.WriteOnly);assert im.save(buf,'PNG');return bytes(buf.data())
def serial(p):
 if p is None:return None
 return dict(samples=[dict(station=s.station,x=s.x,y=s.y,elevation=s.elevation)for s in p.samples],length=p.length,complete=p.complete,min=p.min_elevation(),max=p.max_elevation(),gain=p.total_gain(),slopes=[list(v)for v in p.slopes()])
def provider(name,encoding='terrarium'):return DEMSource(name,name,base+'/'+name+'/{z}/{x}/{y}.png',encoding,15)
def profile_button(dock):next(b for b in dock.findChildren(QPushButton)if b.text()=='Profile selection').click()
def attach(dock,sampler):dock._sampler=sampler;dock._sampler_datum=sampler.datum;sampler.changed.connect(dock._recompute)
def export_csv(dock,path):
 previous=file_dialogs.getSaveFileName;file_dialogs.getSaveFileName=lambda*a,**k:(str(path),'')
 try:next(b for b in dock.findChildren(QPushButton)if b.text()=='Export CSV…').click()
 finally:file_dialogs.getSaveFileName=previous
states=[];cases=[];transport=[];windows=[];samplers=[]

import shutil
initial=args.initial.resolve();data=json.loads((initial/'original-dem.json').read_text());initial_provenance=json.loads((initial/'provenance.json').read_text())
for f in initial.iterdir():
 if f.is_file()and f.suffix in ['.png','.igz','.csv']:shutil.copyfile(f,out/f.name)
shutil.copytree(initial/'disk',out/'disk')
replacements={};guards=[]
try:
 for case in data['cases']:
  name=case['name']
  if name=='provider-switch':continue
  win=QMainWindow();vp=Viewport();win.viewport=vp;win.setCentralWidget(vp);windows.append(win);scene=vp.scene
  igz.load_into(scene,out/case['saved']);assert not scene.selection
  for index in case['selected']:scene.selection.add(scene.geo_paths[index])
  baseline=(len(vp.history.undo_stack),len(vp.history.redo_stack),scene.version)
  snapshot=[g.to_dict()for g in scene.geo_paths]
  dock=ProfileDock(win);win.addDockWidget(Qt.BottomDockWidgetArea,dock);dock.show()
  spec=case['provider'];src=DEMSource(spec['id'],spec['name'],base+spec['path_template'],spec['encoding'],spec['max_zoom'])
  cache=TileCache(out/'disk'/name);sampler=DEMSampler(scene.georef,src,case['zoom'],cache,parent=dock);samplers.append(sampler);attach(dock,sampler)
  # Only missing data is network-visible; every existing disk tile must hit cache.
  for tile in case['tiles']:
   key=src.url(tile['x'],tile['y'],tile['z']).removeprefix(base);gate=threading.Event();gate.set()
   routes[key]=(gate,b'missing',404)
  failures=[];sampler._ensure_fetcher().tileFailed.connect(lambda sid,x,y,z,reason:failures.append(dict(id=sid,x=x,y=y,z=z,reason=reason)))
  start=len(requests);local=[]
  for phase in ['reader-cache-restart','reader-cache-repeat']:
   profile_button(dock);wait(lambda:not sampler._fetcher._inflight,'restart profile quiescence')
   csvfile=name+'-'+phase+'.csv';export_csv(dock,out/csvfile);wait(lambda:not sampler._fetcher._inflight,'CSV retry quiescence')
   local.append(dict(case=name,phase=phase,profile=serial(dock.view._profile),message=dock.view._message,sampler=type(sampler).__name__,path=scene.geo_paths.index(dock._geopath),undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),version=scene.version,provider_id=src.id,arrived=[dict(x=x,y=y,z=z)for x,y,z in sorted(sampler._fields)],csv=csvfile))
   assert baseline==(len(vp.history.undo_stack),len(vp.history.redo_stack),scene.version)and snapshot==[g.to_dict()for g in scene.geo_paths]
  if name!='missing-corrupt':assert len(requests)==start and local[0]['profile']['complete']
  else:assert failures and not local[0]['profile']['complete']
  replacements[name]=local
  guards.append(dict(case=name,selected_after_load=True,readonly_baseline=list(baseline),readonly_unchanged=True,request_paths=requests[start:].copy(),failures=failures,quiescent_after_profile_and_CSV=True))
 final=[]
 for row in data['states']:
  if row['phase']=='reader-cache-restart':final.extend(replacements[row['case']])
  else:final.append(row)
 data['states']=final
 (out/'original-dem.json').write_text(json.dumps(data,indent=2)+'\n');shutil.copyfile(initial/'transport.json',out/'transport.json')
 (out/'restart-observations.json').write_text(json.dumps(dict(initial_reader_one_path_fallback_observed=True,guards=guards,source_retry_policy='404 requests may retry on explicit profile/CSV; no fabricated suppression'),indent=2)+'\n')
 (out/'provenance.json').write_text(json.dumps(dict(source_pin=pin,initial_producer_sha256=initial_provenance['producer_sha256'],initial_original_dem_sha256=hashlib.sha256((initial/'original-dem.json').read_bytes()).hexdigest(),restart_producer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),source_files=initial_provenance['source_files'],scope='Only four actual reader/cache restart pairs updated; all initial and async/switch observations retained literally'),indent=2)+'\n')
 print(json.dumps(dict(cases=len(data['cases']),states=len(final),restart_pairs=len(guards),network_requests=len(requests))))
finally:
 for response in routes.values():response[0].set()
 for sampler in samplers:
  if sampler._fetcher:sampler._fetcher.cancel_all()
 app.processEvents()
 for win in windows:win.close()
 server.shutdown();server.server_close();thread.join(timeout=5)
