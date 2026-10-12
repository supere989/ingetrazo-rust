#!/usr/bin/env python3
"""Actual original reader/sampler re-profile each native saved document, offline."""
import os,sys,json,hashlib
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from views.viewport import Viewport
from views.profile_panel import ProfileDock
from formats import igz
out=Path(sys.argv[1]);app=QApplication([]);original=json.loads((out/'original-profile.json').read_text());rows=[]
def state(path,case):
 w=QMainWindow();vp=Viewport();w.setCentralWidget(vp);w.viewport=vp;igz.load_into(vp.scene,path);s=vp.scene
 for i in case['selected']:s.selection.add(s.geo_paths[i])
 dock=ProfileDock(w);dock.compute_from_selection();p=dock.view._profile
 result=dict(points=[g.to_dict()for g in s.geo_paths],datum=s.georef.to_dict()if s.georef else None,units=s.units,message=dock.view._message,sampler=type(dock._sampler).__name__ if dock._sampler else None,profile=None)
 if p is not None:result['profile']=dict(samples=[dict(station=v.station,x=v.x,y=v.y,elevation=v.elevation)for v in p.samples],length=p.length,complete=p.complete,min=p.min_elevation(),max=p.max_elevation(),gain=p.total_gain(),slopes=[list(t)for t in p.slopes()])
 w.close();return result
for c in original['cases']:n=c['name'];rows.append(dict(case=n,original=state(out/(n+'-original.igz'),c),native=state(out/(n+'-native.igz'),c)))
(out/'profile-reopens.json').write_text(json.dumps(rows,indent=2)+'\n')
