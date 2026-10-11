"""Capture only previously uncovered pinned sequential solid workflows."""
import json,sys,os
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from tests import test_pushpull_subtractive as source
app=QApplication.instance() or QApplication([])
def fields(m):
 return [{"vertices":[[v.x(),v.y(),v.z()] for v in f.vertices],"holes":[[[v.x(),v.y(),v.z()] for v in h] for h in f.holes],"attrs":f.attrs,"interior":bool(f.interior)} for f in m.faces]
rows=[];workflow="";step=0
for symbol in ["_push","_push_real"]:
 original=getattr(source,symbol)
 def wrapper(scene,face,distance,original=original,symbol=symbol):
  global step
  before=fields(scene.mesh);index=scene.mesh.faces.index(face)
  result=original(scene,face,distance)
  rows.append({"name":workflow+"-"+str(step),"workflow":workflow,"step":step,"source":before,"face":index,"distance":distance,"ctrl":False,"result":fields(scene.mesh),"source_handler":symbol})
  step+=1;return result
 setattr(source,symbol,wrapper)
results=[]
for name in ["test_repush_top_extends_holed_wall_keeping_opening","test_stacked_block_side_pushes_stay_clean","test_push_through_wall_makes_a_real_hole"]:
 workflow=name;step=0
 try:getattr(source,name)();error=None
 except Exception as e:error=type(e).__name__+": "+str(e)
 results.append({"workflow":name,"source_test_error":error})
Path(sys.argv[1]).write_text(json.dumps({"cases":rows,"source_tests":results},indent=2)+"\n")
print(json.dumps(results,indent=2))
