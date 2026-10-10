import sys,json,subprocess
from pathlib import Path
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from georef.geoimport import load_features
from georef.datum import SceneDatum
from georef.geopath import GeoPath
from core.scene import Scene
from core.history import History,AddGeoPathCommand,CompoundCommand
from PySide6.QtGui import QVector3D
file=Path(sys.argv[1]);feats=load_features(file);expected=[]
for seeded in [False,True]:
 scene=Scene();scene.mesh.vertex(QVector3D(9,8,7))
 pts=[p for f in feats for p in f.points]
 d=SceneDatum(-13.5,-71.9,1234.,23.) if seeded else SceneDatum(sum(p[0] for p in pts)/len(pts),sum(p[1] for p in pts)/len(pts))
 scene.georef=d;history=History(scene)
 commands=[AddGeoPathCommand(GeoPath([d.geodetic_to_local(la,lo)for la,lo in f.points],closed=f.closed,name=f.name))for f in feats]
 history.execute(commands[0]if len(commands)==1 else CompoundCommand(commands));states=[]
 for phase in ['import','undo','redo']:
  if phase=='undo':history.undo()
  if phase=='redo':history.redo()
  states.append({'phase':phase,'datum':[d.lat,d.lon,d.alt,d.north],'paths':[{'name':p.name,'closed':p.closed,'points':[[q.x(),q.y(),q.z()]for q in p.points]}for p in scene.geo_paths],'vertices':len(scene.mesh.vertices)})
 expected.append(states)
actual=json.loads(subprocess.run([sys.argv[2],str(file)],capture_output=True,text=True,check=True).stdout)
assert actual==expected,(actual,expected)
root=Path(sys.argv[3]);root.mkdir(parents=True,exist_ok=True)
for name,data in [('original',expected),('native',actual)]: (root/(name+'.json')).write_text(json.dumps(data,indent=2)+'\n')
print(json.dumps({'exact_states':6,'scope':'original datum projection and path Command/History vs native app; no rendered dialogs/base-map/camera claim'}))
