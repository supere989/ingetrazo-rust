import sys,json,subprocess
from pathlib import Path
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from georef.geoimport import load_features
from georef.datum import SceneDatum
from georef.geopath import GeoPath
from core.scene import Scene
from core.history import History,AddGeoPathCommand,CompoundCommand
from PySide6.QtGui import QVector3D
from core.camera import OrbitCamera
from views.main_window import MainWindow
from types import SimpleNamespace
assert subprocess.check_output(['git','-C','ingetrazo','rev-parse','HEAD'],text=True).strip()=='6be29fe437a7cd992d4e079f71720821b25216b9'
file=Path(sys.argv[1]);feats=load_features(file);expected=[]
for seeded in [False,True]:
 scene=Scene();scene.mesh.vertex(QVector3D(9,8,7));scene.geo_paths.append(GeoPath([QVector3D(4,5,6),QVector3D(7,8,9)],closed=False,name="Existing"))
 pts=[p for f in feats for p in f.points]
 d=SceneDatum(-13.5,-71.9,1234.,23.) if seeded else SceneDatum(sum(p[0] for p in pts)/len(pts),sum(p[1] for p in pts)/len(pts))
 scene.georef=d;history=History(scene)
 commands=[AddGeoPathCommand(GeoPath([d.geodetic_to_local(la,lo)for la,lo in f.points],closed=f.closed,name=f.name))for f in feats]
 history.execute(commands[0]if len(commands)==1 else CompoundCommand(commands));camera=OrbitCamera();MainWindow._frame_geo_paths(SimpleNamespace(viewport=SimpleNamespace(camera=camera,update=lambda:None)),scene.geo_paths);states=[]
 for phase in ['import','undo','redo']:
  if phase=='undo':history.undo()
  if phase=='redo':history.redo()
  states.append({'phase':phase,'datum':[d.lat,d.lon,d.alt,d.north],'paths':[{'name':p.name,'closed':p.closed,'points':[[q.x(),q.y(),q.z()]for q in p.points]}for p in scene.geo_paths],'vertices':len(scene.mesh.vertices),'camera':{'target':[camera.target.x(),camera.target.y(),camera.target.z()],'distance':camera.distance,'yaw':camera.yaw,'pitch':camera.pitch,'perspective':camera.perspective}})
 expected.append(states)
actual=json.loads(subprocess.run([sys.argv[2],str(file)],capture_output=True,text=True,check=True).stdout)
assert actual==expected,(actual,expected)
root=Path(sys.argv[3]);root.mkdir(parents=True,exist_ok=True)
for name,data in [('original',expected),('native',actual)]: (root/(name+'.json')).write_text(json.dumps(data,indent=2)+'\n')
print(json.dumps({'exact_states':6,'scope':'original datum projection and path Command/History vs native app; actual original camera framing method; no rendered dialogs/base-map claim'}))
