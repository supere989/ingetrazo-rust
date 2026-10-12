#!/usr/bin/env python3
"""One offline original ProfileDock/PhotoMeshSampler corpus; no DEM network or GPU claim."""
import os,sys,json,hashlib
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
import numpy as np
from PySide6.QtWidgets import QApplication,QMainWindow,QPushButton
from PySide6.QtGui import QVector3D as V
from views.viewport import Viewport
from views.profile_panel import ProfileDock
from views.filedialogs import file_dialogs
from georef.photomesh import PhotoMesh,PhotoMeshSampler
from georef.geopath import GeoPath
from georef.datum import SceneDatum
from core.layers import Layer
from formats import igz
out=Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True);app=QApplication([]);cases=[];rows=[]
rect=[[0,0,0],[10,0,5],[0,10,2.5],[10,10,7.5]];tris=[[0,1,2],[1,3,2]]
recipes=[
 ('long-sloped',[[0,0,0],[1000,0,100],[0,1000,200],[1000,1000,150]],tris,[[[-10,.125,7],[1100,.125,9]]],False,[],True,'odm'),
 ('closed-selected',rect,tris,[[[1,1,0],[7,1,0],[7,7,0]],[[2,2,0],[3,2,0]]],True,[0],True,'local'),
 ('highest-gaps',[[0,0,0],[16,0,8],[0,16,4],[0,0,10],[16,0,10],[0,16,10],[4,4,-20],[4,4,20],[4,10,0]],[[0,1,2],[3,4,5],[6,7,8]],[[[-2,0,0],[8,0,0],[8,8,0]]],False,[],True,'odm'),
 ('hidden-survey-layer',rect,tris,[[[1,1,0],[9,1,0]]],False,[],True,'dem'),
 ('ambiguous',rect,tris,[[[1,1,0],[7,1,0]],[[2,2,0],[3,2,0]]],False,[],True,'local'),
 ('no-datum',rect,tris,[[[1,1,0],[7,1,0]]],False,[],False,'odm'),
 ('vertical-only',[[1,1,0],[1,1,5],[1,7,3]],[[0,1,2]],[[[1,1,0],[7,1,0]]],False,[],True,'odm'),
 ('no-path',rect,tris,[],False,[],True,'local'),
 ('zero-span',rect,tris,[[[1,1,0],[1,1,5]]],False,[],True,'local')]
if len(sys.argv)>2:recipes=[r for r in recipes if r[0]==sys.argv[2]]
def profile(p):
 if p is None:return None
 return dict(samples=[dict(station=s.station,x=s.x,y=s.y,elevation=s.elevation)for s in p.samples],length=p.length,complete=p.complete,min=p.min_elevation(),max=p.max_elevation(),gain=p.total_gain(),slopes=[list(v)for v in p.slopes()])
for name,vertices,triangles,paths,closed,selected,datum,vertical in recipes:
 win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp;s=vp.scene
 s.units={'length':'ft-in','precision':2}
 if datum:s.georef=SceneDatum(-12,-76,1234.25,north=17)
 s.photo_mesh=PhotoMesh(np.asarray(vertices,dtype=np.float32).reshape(-1,3),np.zeros((len(vertices),2),dtype=np.float32),np.asarray(triangles,dtype=np.uint32).reshape(-1,3),[])
 s.photo_mesh.vertical_reference=vertical
 if name=='hidden-survey-layer':s.photo_mesh.layer='Survey';s.layers.append(Layer('Survey',visible=False))
 s.geo_paths=[GeoPath([V(*p)for p in ps],closed=closed and i==0,name='Route '+str(i))for i,ps in enumerate(paths)]
 for i in selected:s.selection.add(s.geo_paths[i])
 igz.save_scene(s,out/(name+'-input.igz'))
 if name=='vertical-only':
  igz.load_into(s,out/(name+'-input.igz'))
  assert s.photo_mesh is not None and s.photo_mesh.triangle_count==1
 dock=ProfileDock(win);win.addDockWidget(__import__('PySide6.QtCore',fromlist=['Qt']).Qt.BottomDockWidgetArea,dock);dock.show()
 before=(len(vp.history.undo_stack),len(vp.history.redo_stack),s.version)
 next(b for b in dock.findChildren(QPushButton)if b.text()=='Profile selection').click();app.processEvents()
 def record(phase):rows.append(dict(case=name,phase=phase,profile=profile(dock.view._profile),message=dock.view._message,sampler=type(dock._sampler).__name__ if dock._sampler else None,path=s.geo_paths.index(dock._geopath)if dock._geopath in s.geo_paths else None,undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),version=s.version))
 record('compute');next(b for b in dock.findChildren(QPushButton)if b.text()=='Profile selection').click();record('repeat')
 if dock._geopath is not None:
  old=file_dialogs.getSaveFileName;file_dialogs.getSaveFileName=lambda *a,**k:(str(out/(name+'-original.csv')),'')
  try:next(b for b in dock.findChildren(QPushButton)if b.text()=='Export CSV…').click()
  finally:file_dialogs.getSaveFileName=old
 assert before==(len(vp.history.undo_stack),len(vp.history.redo_stack),s.version)
 igz.save_scene(s,out/(name+'-original.igz'))
 if name=='highest-gaps':
  # Visible flag alone controls source sampler choice; don't fetch DEM data.
  s.photo_mesh.visible=False;fallback=dock._ensure_sampler(s.georef);assert type(fallback).__name__=='DEMSampler';(out/'dem-boundary.json').write_text(json.dumps(dict(visible_photo_first=True,hidden_photo_selects_dem=True,network_not_started=fallback._fetcher is None)))
 cases.append(dict(name=name,selected=selected,csv=dock._geopath is not None,vertical=vertical));win.close()
(out/'original-profile.json').write_text(json.dumps(dict(cases=cases,states=rows,numpy=np.__version__,source_pin='6be29fe437a7cd992d4e079f71720821b25216b9'),indent=2)+'\n');print(json.dumps(dict(cases=len(cases),states=len(rows),actual_profile_dock=True,actual_photo_sampler=True,network=False)))
