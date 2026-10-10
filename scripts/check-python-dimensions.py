#!/usr/bin/env python3
import os,sys,json,math,tempfile,subprocess
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from core.dimension import Dimension
from core.scene import Scene
from formats import igz
def main():
 app=QApplication.instance() or QApplication([]);cases=[]
 for degrees in (14.9,15.1,30,45,74.9,75.1):
  a=[0,0,0];b=[4*math.cos(math.radians(degrees)),4*math.sin(math.radians(degrees)),0]
  for p in ([6,1,0],[1,6,0],[-4,5,0],[6,6,0],[2,2,3]):cases.append([a,b,p])
 cases+=[[[0,0,0],[0,0,0],[2,3,4]],[[1,2,3],[3,5,7],[8,1,9]]]
 with tempfile.TemporaryDirectory() as tmp:
  directory=Path(tmp);(directory/"cases.json").write_text(json.dumps(cases));(directory/"source.igz").write_text(json.dumps({"igz_format":1,"app_version":"independent","scene":{"edges":[],"faces":[],"dimensions":[{"a":[0,0,0],"b":[2,3,0],"offset":[0,1,0],"text":"Source <>","vendor_dimension":{"preserve":37}}]}}))
  subprocess.run(["cargo","run","--locked","-p","ingetrazo-formats","--example","dimension_acceptance_fixture","--",str(directory)],check=True)
  rows=json.loads((directory/"placements.json").read_text())
  for inputs,row in zip(cases,rows):
   offset,axis=Dimension.placement_for_cursor(*[V(*p) for p in inputs]);assert axis==row["axis"],(inputs,axis,row)
   assert all(abs(a-b)<1e-5 for a,b in zip((offset.x(),offset.y(),offset.z()),row["offset"])),(inputs,row,offset)
  raw=json.loads((directory/"edited.igz").read_text());assert raw["scene"]["dimensions"][0]["vendor_dimension"]=={"preserve":37}
  reopened=Scene();igz.load_into(reopened,directory/"edited.igz");assert len(reopened.dimensions)==2;d=reopened.dimensions[0]
  assert d.a==V(1,2,3) and d.b==V(4,6,3) and d.offset==V(-2,0,0);assert d.axis=="y" and d.text=="Span <>" and d.layer=="Dimension layer"
 report={"provenance":context(),"passed":True,"original_cursor_cases":len(cases),"scope":"Original placement offsets/axis across 15-degree thresholds and side/corner/aligned/3D/zero-length cases; fresh and edited native dimension a/b/offset/axis/text/layer original runtime reopen and opaque metadata retention. Anchor tracking and rendered text remain open."}
 Path("docs/audits/python-dimension-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
