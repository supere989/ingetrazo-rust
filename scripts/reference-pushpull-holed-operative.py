"""Missing operative classification witness; untouched pinned original objects."""
import sys,os,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from tests import test_pushpull_subtractive as t
app=QApplication.instance() or QApplication([]);old=t._push;step=0;record={}
def fields(m):return [{"vertices":[[v.x(),v.y(),v.z()]for v in f.vertices],"holes":[[[v.x(),v.y(),v.z()]for v in h]for h in f.holes],"attrs":f.attrs,"interior":f.interior}for f in m.faces]
def wrapped(s,f,d):
 global step,record
 if step==2:
  record={"source":fields(s.mesh),"face":s.mesh.faces.index(f),"distance":d,"ctrl":False}
  result=t._push_real(s,f,d);record["result"]=fields(s.mesh)
 else:result=old(s,f,d)
 step+=1;return result
t._push=wrapped;t.test_repush_top_extends_holed_wall_keeping_opening()
record["name"]="operative-holed-wall-repush";Path(sys.argv[1]).write_text(json.dumps({"cases":[record]},indent=2)+"\n")
