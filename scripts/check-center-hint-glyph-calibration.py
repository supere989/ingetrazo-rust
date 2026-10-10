from pathlib import Path
import json,hashlib,sys
from PySide6.QtGui import QImage
from center_hint_glyph import center_hint_metrics
root=Path(sys.argv[1]);rows=[]
for name,cx,cy,want in [("cloud-center",500,273,True),("cloud-endpoint",500,273,False),("native-center",661,337,True),("native-tiny",661,337,False),("native-malformed",661,337,False),("source-tiny",500,273,False),("source-malformed",500,273,False)]:
 path=root/(name+".png");im=QImage(str(path));assert not im.isNull()and (im.width(),im.height())==((1200,800)if name.startswith("native")else(1000,700))
 def pixel(x,y):
  c=im.pixelColor(x,y);return (c.red(),c.green(),c.blue())
 row=center_hint_metrics(pixel,cx,cy);assert row["filled_center"]==want,(name,row)
 rows.append({"name":name,"center":[cx,cy],"sha256":hashlib.sha256(path.read_bytes()).hexdigest(),**row})
assert rows[0]["core_green"]==17 and rows[1]["core_green"]==3 and rows[1]["outer_green"]==115
assert rows[2]["core_green"]==24
observation=json.loads((root/"source-tiny-hint-observation.json").read_text())["center_hint_observation"];assert observation["radius_px"]==0 and observation["allowed"]is False
# Geometric/palette controls distinguish opaque fill from sparse inference/endpoint ink.
synthetic=[]
for name,points,want in [("empty",set(),False),("thin-axis",{(x,0)for x in range(-2,3)},False),("thin-cross",{(x,0)for x in range(-2,3)}|{(0,y)for y in range(-2,3)},False),("endpoint-core-speck",{(-1,-1),(-1,0),(0,0)},False),("opaque-fill",{(x,y)for x in range(-2,3)for y in range(-2,3)},True)]:
 row=center_hint_metrics(lambda x,y:(0,180,70)if(x,y)in points else(210,210,210),0,0);assert row["filled_center"]==want
 synthetic.append({"name":name,**row})
report={"passed":True,"actual_png_cases":rows,"source_tiny_admission":observation,"synthetic_geometry_controls":synthetic,"boundary":"Opaque radius4 center dot classification in frozen unpainted UI fixture/palette, not a universal green-ink classifier or rendering tolerance. Actual endpoint/sparse geometry may produce green ink; absence of any green is not absence of center hint."}
(root/"result.json").write_text(json.dumps(report,indent=2));print(json.dumps(report))
