import sys,json,subprocess
from pathlib import Path
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from georef.geoimport import load_features
from formats.igz import load_into
from georef.datum import SceneDatum
from georef.geopath import GeoPath
from core.scene import Scene
from core.history import History,AddGeoPathCommand,CompoundCommand
from PySide6.QtGui import QVector3D
from core.camera import OrbitCamera
from views.main_window import MainWindow
from types import SimpleNamespace
from PySide6.QtWidgets import QApplication,QSpinBox,QCheckBox,QLabel,QDoubleSpinBox
from views.tray import BaseMapPanel
from georef.tiles import PRESETS,DEFAULT_SOURCE_ID
app=QApplication.instance() or QApplication([])
assert subprocess.check_output(['git','-C','ingetrazo','rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
file=Path(sys.argv[1]);feats=load_features(file);expected=[]
for seeded in [False,True]:
 scene=Scene();scene.mesh.vertex(QVector3D(9,8,7));scene.geo_paths.append(GeoPath([QVector3D(4,5,6),QVector3D(7,8,9)],closed=False,name="Existing"))
 pts=[p for f in feats for p in f.points]
 d=SceneDatum(-13.5,-71.9,1234.,23.) if seeded else SceneDatum(sum(p[0] for p in pts)/len(pts),sum(p[1] for p in pts)/len(pts))
 scene.georef=d;history=History(scene)
 commands=[AddGeoPathCommand(GeoPath([d.geodetic_to_local(la,lo)for la,lo in f.points],closed=f.closed,name=f.name))for f in feats]
 history.execute(commands[0]if len(commands)==1 else CompoundCommand(commands))
 panel=SimpleNamespace(_window=SimpleNamespace(viewport=SimpleNamespace(scene=scene,reset_tiles=lambda:None)),_zoom=QSpinBox(),_show=QCheckBox(),_attribution=QLabel(),_lat=QDoubleSpinBox(),_lon=QDoubleSpinBox(),_current_source=lambda:PRESETS[DEFAULT_SOURCE_ID],_sync_utm_from_ll=lambda:None)
 panel._zoom.setRange(1,21);panel._zoom.setValue(16);panel._show.setChecked(True)
 panel._capture_around=lambda *args:BaseMapPanel._capture_around(panel,*args)
 BaseMapPanel.setup_for_import(panel,d,scene.geo_paths)
 camera=OrbitCamera();MainWindow._frame_geo_paths(SimpleNamespace(viewport=SimpleNamespace(camera=camera,update=lambda:None)),scene.geo_paths);states=[]
 for phase in ['import','undo','redo']:
  if phase=='undo':history.undo()
  if phase=='redo':history.redo()
  states.append({'phase':phase,'datum':[d.lat,d.lon,d.alt,d.north],'paths':[{'name':p.name,'closed':p.closed,'points':[[q.x(),q.y(),q.z()]for q in p.points]}for p in scene.geo_paths],'vertices':len(scene.mesh.vertices),'tile_layer':scene.tile_layer.to_dict(),'camera':{'target':[camera.target.x(),camera.target.y(),camera.target.z()],'distance':camera.distance,'yaw':camera.yaw,'pitch':camera.pitch,'perspective':camera.perspective}})
 expected.append(states)
actual=json.loads(subprocess.run([sys.argv[2],str(file),str(Path(sys.argv[3])/"native-reopens")],capture_output=True,text=True,check=True).stdout)
assert actual==expected,(actual,expected)
for seeded,row in zip([False,True],expected):
 reopened=Scene();load_into(reopened,Path(sys.argv[3])/"native-reopens"/(str(seeded).lower()+".igz"))
 assert reopened.tile_layer.to_dict()==row[-1]["tile_layer"]
root=Path(sys.argv[3]);root.mkdir(parents=True,exist_ok=True)
for name,data in [('original',expected),('native',actual)]: (root/(name+'.json')).write_text(json.dumps(data,indent=2)+'\n')
print(json.dumps({'exact_states':6,'scope':'original datum projection and path Command/History vs native app; actual original camera framing method; actual original base-map capture methods and original native-save reopen; no imagery/UI claim'}))
