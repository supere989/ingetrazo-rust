import os,sys,json,subprocess
from pathlib import Path
from types import SimpleNamespace
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QSpinBox,QCheckBox,QLabel,QDoubleSpinBox
from core.scene import Scene
from core.history import History,AddGeoPathCommand,CompoundCommand
from georef.datum import SceneDatum
from georef.geoimport import load_features
from georef.geopath import GeoPath
from georef.tiles import PRESETS,DEFAULT_SOURCE_ID
from views.tray import BaseMapPanel
app=QApplication([]);scene=Scene();history=History(scene)
p=SimpleNamespace(_window=SimpleNamespace(viewport=SimpleNamespace(scene=scene,reset_tiles=lambda:None)),_zoom=QSpinBox(),_show=QCheckBox(),_attribution=QLabel(),_lat=QDoubleSpinBox(),_lon=QDoubleSpinBox(),_current_source=lambda:PRESETS[DEFAULT_SOURCE_ID],_sync_utm_from_ll=lambda:None)
p._zoom.setRange(1,21);p._zoom.setValue(16);p._show.setChecked(True);p._capture_around=lambda *args:BaseMapPanel._capture_around(p,*args)
expected={}
for index,file in enumerate(sys.argv[2:4]):
 if index:history.undo()
 feats=load_features(file)
 if scene.georef is None:
  pts=[x for f in feats for x in f.points];scene.georef=SceneDatum(sum(x[0]for x in pts)/len(pts),sum(x[1]for x in pts)/len(pts))
 cmds=[AddGeoPathCommand(GeoPath([scene.georef.geodetic_to_local(*pt)for pt in f.points],closed=f.closed,name=f.name))for f in feats];history.execute(cmds[0]if len(cmds)==1 else CompoundCommand(cmds));BaseMapPanel.setup_for_import(p,scene.georef,scene.geo_paths)
 expected['first'if index==0 else'second']=scene.tile_layer.to_dict()
actual=json.loads(subprocess.run([sys.argv[1],*sys.argv[2:4]],capture_output=True,text=True,check=True).stdout)
root=Path(sys.argv[4]);root.mkdir(parents=True,exist_ok=True)
for name,data in [('original',expected),('native',actual)]: (root/(name+'.json')).write_text(json.dumps(data,indent=2)+'\n')
assert actual==expected,(actual,expected)
print(json.dumps({'exact_captures':2,'requested_zoom':p._zoom.value(),'reduced_capture_zoom':expected['first']['zoom'],'second_capture_zoom':expected['second']['zoom']}))
