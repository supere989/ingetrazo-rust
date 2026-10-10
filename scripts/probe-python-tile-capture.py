#!/usr/bin/env python3
"""Actual pinned IGZ map-block admission; no tile fetch/render claim."""
import sys,json,os
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from core.scene import Scene
from formats import igz
root=Path(sys.argv[1]);root.mkdir(parents=True,exist_ok=True)
igz.save_scene(Scene(),root/"template.igz");template=json.loads((root/"template.igz").read_text())
cases=[{},{"source":"osm"},{"source":"custom"},{"source":"esri_imagery","name":"ignored","url_template":"ignored"},{"source":"custom","zoom":"12","visible":[],"opacity":1.5},{"source":"custom","zoom":True,"radius_m":"42.5","patches":[]},{"source":"custom","opacity":-2},{"source":"custom","patches":[["1","2","3","4"]]},{"source":"custom","zoom":"12.8"},{"source":"custom","radius_m":None},{"source":"custom","patches":[[1,"bad",3,4]]},{"source":"custom","zoom":300}]
cases.extend([{"source":None},{"source":7},{"source":"custom","name":None,"url_template":12,"attribution":False},{"source":"custom","zoom":"1_2"},{"source":"custom","zoom":"１２"},{"source":"custom","radius_m":"1_200.5"},{"source":"custom","patches":["1234"]},{"source":"custom","patches":[{"1":0,"2":0}]},{"source":"custom","patches":[[1,2,3]]},{"source":"custom","opacity":True},{"source":"custom","zoom":-2.8}])
rows=[]
for i,value in enumerate(cases):
 document=json.loads(json.dumps(template));document["scene"]["tile_layer"]=value
 path=root/f"case-{i}.igz";path.write_text(json.dumps(document))
 scene=Scene();igz.load_into(scene,path)
 rows.append({"input":value,"document_opened":True,"tile_capture":None if scene.tile_layer is None else scene.tile_layer.to_dict()})
(root/"original.json").write_text(json.dumps({"cases":rows},indent=2)+"\n")
print(f"PASS {len(cases)} actual original map-block document opens")
