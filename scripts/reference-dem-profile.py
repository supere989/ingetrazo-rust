#!/usr/bin/env python3
"""Finite actual pinned Qt DEM/ProfileDock HTTP and disk-cache source observation."""
import argparse, hashlib, io, json, os, subprocess, sys, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);args=parser.parse_args()
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
def run_case(name,mode,encoding='terrarium'):
 win=QMainWindow();vp=Viewport();win.viewport=vp;win.setCentralWidget(vp);windows.append(win);scene=vp.scene
 lat,lon=num2deg(2389,4325,13);datum=SceneDatum(lat,lon,37.5,north=30.)
 fractions=[(2389.2,4325.2),(2389.3,4325.25)]if mode in ['cache','mapbox','switch']else[(2388.85,4324.85),(2389.15,4325.15)]
 pts=[]
 for x,y in fractions:
  la,lo=num2deg(x,y,13);pts.append(datum.geodetic_to_local(la,lo,datum.alt))
 scene.georef=datum;scene.geo_paths=[GeoPath(pts,False,name)];scene.selection.add(scene.geo_paths[0]);scene.units={'length':'m','precision':3}
 igz.save_scene(scene,out/(name+'-input.igz'))
 dock=ProfileDock(win);win.addDockWidget(Qt.BottomDockWidgetArea,dock);dock.show()
 cache=TileCache(out/'disk'/name);src=provider(name,encoding);sampler=DEMSampler(datum,src,13,cache,parent=dock);samplers.append(sampler);attach(dock,sampler)
 lats=[];lons=[]
 for p in pts:la,lo,_=datum.local_to_geodetic(p);lats.append(la);lons.append(lo)
 keys=[(x,y,13)for x,y in tiles_covering(min(lats),min(lons),max(lats),max(lons),13)];tiles=[];gates={};failures=[];changed=[];variants=[]
 sampler.changed.connect(lambda:changed.append(len(sampler._fields)))
 before=(len(vp.history.undo_stack),len(vp.history.redo_stack),scene.version)
 start=len(requests)
 for index,(x,y,z)in enumerate(keys):
  data=png(encoding,index);file=name+'-'+str(x)+'-'+str(y)+'.png';(out/file).write_bytes(data);tiles.append(dict(x=x,y=y,z=z,file=file))
  gate=threading.Event();gates[(x,y,z)]=gate;code=404 if mode=='failure'and index==len(keys)-1 else 200
  routes['/'+name+'/'+str(z)+'/'+str(x)+'/'+str(y)+'.png']=(gate,data if code==200 else b'missing',code)
  if mode in ['cache','mapbox']:cache.put(src.id,x,y,z,data);gate.set()
  if mode=='failure'and index==0:cache.put(src.id,x,y,z,b'corrupt image cache')
 def record(phase,csv=False,current=None):
  current=current or dock._sampler
  csvfile=None
  if csv:csvfile=name+'-'+phase+'.csv';export_csv(dock,out/csvfile)
  states.append(dict(case=name,phase=phase,profile=serial(dock.view._profile),message=dock.view._message,sampler=type(dock._sampler).__name__,path=scene.geo_paths.index(dock._geopath)if dock._geopath in scene.geo_paths else None,undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),version=scene.version,provider_id=current.source.id,arrived=[dict(x=x,y=y,z=z)for x,y,z in sorted(current._fields)],csv=csvfile))
 profile_button(dock);record('initial',csv=True)
 if mode not in ['cache','mapbox']:
  wait(lambda:len(requests)-start==len(keys),'initial HTTP starts')
  sampler._fetcher.tileFailed.connect(lambda sid,x,y,z,reason:failures.append(dict(id=sid,x=x,y=y,z=z,category='HTTP'if'not found'in reason.lower()or'404'in reason else'network',reason=reason)))
  if mode=='switch':
   new=provider(name+'-new','mapbox');second=DEMSampler(datum,new,13,cache,parent=dock);samplers.append(second)
   newgate=threading.Event()
   for index,(x,y,z)in enumerate(keys):
    data=png('mapbox',index+3);file=name+'-new-'+str(x)+'-'+str(y)+'.png';(out/file).write_bytes(data);variants.append(dict(x=x,y=y,z=z,file=file));routes['/'+new.id+'/'+str(z)+'/'+str(x)+'/'+str(y)+'.png']=(newgate,data,200)
   attach(dock,second);profile_button(dock);record('new-provider-gaps')
   for gate in gates.values():gate.set()
   wait(lambda:len(sampler._fields)==len(keys),'old provider completion');record('old-provider-arrived',current=second)
   assert not second._fields and dock.view._profile.complete is False
   newgate.set();wait(lambda:len(second._fields)==len(keys),'new provider completion');record('new-provider-arrived',True)
   transport.append(dict(case=name,old_response_retired=False,old_fields=len(sampler._fields),new_fields=len(second._fields),scope='Actual DEMSampler API owner replacement; old fetcher is not canceled by ProfileDock replacement'))
  else:
   first=keys[0];gates[first].set();wait(lambda:first in sampler._fields,'first tile completion');record('first-arrival')
   for gate in gates.values():gate.set()
   expected=len(keys)-(1 if mode=='failure'else 0);wait(lambda:len(sampler._fields)==expected and not sampler._fetcher._inflight,'all responses');record('all-responses',True)
   if mode=='failure':assert failures and not dock.view._profile.complete
   else:assert dock.view._profile.complete
  if sampler._fetcher:sampler._fetcher.cancel_all();wait(lambda:not sampler._fetcher._inflight,'owned fetcher retirement')
 record('repeat',True);profile_button(dock);record('recompute');assert before==(len(vp.history.undo_stack),len(vp.history.redo_stack),scene.version)
 if mode!='switch':
  igz.save_scene(scene,out/(name+'-original.igz'));igz.load_into(scene,out/(name+'-original.igz'))
  restart_start=len(requests)
  fresh=DEMSampler(scene.georef,src,13,TileCache(out/'disk'/name),parent=dock);samplers.append(fresh);attach(dock,fresh);profile_button(dock);record('reader-cache-restart',True)
  if mode!='failure':assert dock.view._profile.complete and len(requests)==restart_start
  if fresh._fetcher:fresh._fetcher.cancel_all()
 else:igz.save_scene(scene,out/(name+'-original.igz'))
 cases.append(dict(name=name,input=name+'-input.igz',saved=name+'-original.igz',selected=[0],provider=dict(id=src.id,name=src.name,encoding=src.encoding,max_zoom=src.max_zoom,path_template='/'+src.id+'/{z}/{x}/{y}.png'),zoom=13,tiles=tiles,provider_variant=dict(id=name+'-new',encoding='mapbox',path_template='/'+name+'-new/{z}/{x}/{y}.png',tiles=variants)if variants else None))
 transport.append(dict(case=name,paths=requests[start:].copy(),bbox_tiles=[dict(x=x,y=y,z=z)for x,y,z in keys],changed=changed,failures=failures,disk_files=[str(p.relative_to(cache.root))for p in sorted(cache.root.rglob('*'))if p.is_file()]))
try:
 for recipe in [('cache-rotated-south','cache'),('async-bbox','async'),('missing-corrupt','failure'),('provider-switch','switch'),('mapbox-decoding','mapbox','mapbox')]:run_case(*recipe)
 result=dict(source_pin=pin,numpy=np.__version__,qt=qVersion(),default_provider=dict(id=AWS_TERRAIN.id,url_template=AWS_TERRAIN.url_template,encoding=AWS_TERRAIN.encoding,max_zoom=AWS_TERRAIN.max_zoom,zoom=13),cases=cases,states=states)
 (out/'original-dem.json').write_text(json.dumps(result,indent=2)+'\n');(out/'transport.json').write_text(json.dumps(dict(cases=transport,max_http_active=peak,owned_loopback_only=True,loopback_base_url=base),indent=2)+'\n')
 (out/'provenance.json').write_text(json.dumps(dict(source_pin=pin,producer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),source_files={n:hashlib.sha256((original/n).read_bytes()).hexdigest()for n in ['georef/dem.py','georef/tile_fetcher.py','georef/tiles.py','georef/profile.py','georef/datum.py','views/profile_panel.py','views/viewport.py','formats/igz.py']}),indent=2)+'\n')
 print(json.dumps(dict(cases=len(cases),states=len(states),requests=len(requests),loopback_only=True)))
finally:
 for response in routes.values():response[0].set()
 for sampler in samplers:
  if sampler._fetcher:sampler._fetcher.cancel_all()
 app.processEvents()
 for win in windows:win.close()
 server.shutdown();server.server_close();thread.join(timeout=5)
