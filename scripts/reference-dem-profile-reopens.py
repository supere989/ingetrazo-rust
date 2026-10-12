#!/usr/bin/env python3
"""Actual original reader + DEMSampler/ProfileDock over native-owned tile cache."""
import os,sys,json,threading,time,shutil
from pathlib import Path
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
source=Path(sys.argv[1]).resolve();out=Path(sys.argv[2]).resolve()
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"ingetrazo"))
from PySide6.QtCore import Qt
from PySide6.QtWidgets import QApplication,QMainWindow
from views.viewport import Viewport
from views.profile_panel import ProfileDock
from georef.dem import DEMSampler,DEMSource
from georef.tiles import TileCache
from formats import igz
app=QApplication([]);windows=[];samplers=[]
class Handler(BaseHTTPRequestHandler):
 def log_message(self,*a):pass
 def do_GET(self):self.send_error(404)
server=ThreadingHTTPServer(("127.0.0.1",0),Handler);server.daemon_threads=True
thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
base=f"http://127.0.0.1:{server.server_port}"
data=json.loads((source/"original-dem.json").read_text())
def state(path,case,cache):
 w=QMainWindow();vp=Viewport();w.viewport=vp;w.setCentralWidget(vp);windows.append(w)
 igz.load_into(vp.scene,path);s=vp.scene
 for i in case["selected"]:s.selection.add(s.geo_paths[i])
 spec=case.get("provider_variant") or case["provider"]
 src=DEMSource(spec["id"],spec["name"],base+spec["path_template"],spec["encoding"],spec["max_zoom"])
 dock=ProfileDock(w);w.addDockWidget(Qt.BottomDockWidgetArea,dock)
 sampler=DEMSampler(s.georef,src,case["zoom"],TileCache(cache),parent=dock);samplers.append(sampler)
 dock._sampler=sampler;dock._sampler_datum=s.georef;sampler.changed.connect(dock._recompute)
 before=(len(vp.history.undo_stack),len(vp.history.redo_stack),s.version)
 dock.compute_from_selection()
 deadline=time.monotonic()+15
 while sampler._fetcher and sampler._fetcher._inflight:
  app.processEvents();time.sleep(.002);assert time.monotonic()<deadline
 p=dock.view._profile
 assert before==(len(vp.history.undo_stack),len(vp.history.redo_stack),s.version)
 return dict(points=[g.to_dict()for g in s.geo_paths],datum=s.georef.to_dict(),units=s.units,
    message=dock.view._message,sampler=type(sampler).__name__,provider_id=src.id,
    profile=None if p is None else dict(samples=[dict(station=v.station,x=v.x,y=v.y,elevation=v.elevation)for v in p.samples],
    length=p.length,complete=p.complete,min=p.min_elevation(),max=p.max_elevation(),gain=p.total_gain(),slopes=[list(t)for t in p.slopes()]))
try:
 rows=[]
 for c in data["cases"]:
  name=c["name"]
  rows.append(dict(case=name,original=state(source/c["saved"],c,source/"disk"/name),
                   native=state(out/(name+"-native.igz"),c,out/"native-disk"/name)))
 (out/"native-reopens.json").write_text(json.dumps(rows,indent=2)+"\n")
 print(json.dumps(dict(reader_pairs=len(rows))))
finally:
 for s in samplers:
  if s._fetcher:s._fetcher.cancel_all()
 app.processEvents()
 for w in windows:w.close()
 server.shutdown();server.server_close();thread.join(timeout=5)
