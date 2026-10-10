#!/usr/bin/env python3
import os,sys,json,tempfile,subprocess,math
from pathlib import Path
import numpy as np
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QVector3D as V
from core.dimension import Dimension
def transform(angle,scale,translation,axis):
 a=math.radians(angle);c,s=math.cos(a),math.sin(a);r=np.eye(4)
 if axis=="z":r[:3,:3]=[[c,-s,0],[s,c,0],[0,0,1]]
 else:r[:3,:3]=[[1,0,0],[0,c,-s],[0,s,c]]
 result=r@np.diag([*scale,1]);result[:3,3]=translation;return result
def vector(matrix,v,point=True):return (matrix@np.array([*v,1 if point else 0]))[:3]
def coords(v):return [v.x(),v.y(),v.z()]
def main():
 parent=transform(20,[1,.75,2],[10,20,30],"x");cases=[]
 for angle,scale in ((0,[1,1,1]),(30,[-2,3,.5]),(75,[.5,-2,4])):
  child=transform(angle,scale,[1,2,3],"z")
  for axis in (None,"x","y","z"):
   cases.append({"parent":parent.flatten(order="F").tolist(),"child":child.flatten(order="F").tolist(),"a":[1,2,3],"b":[4,6,5],"offset":[-2,3,4],"cursor":[8,-2,4],"axis":axis})
 with tempfile.TemporaryDirectory() as tmp:
  directory=Path(tmp);(directory/"world-cases.json").write_text(json.dumps(cases));subprocess.run(["cargo","run","--locked","-p","ingetrazo-gui","--example","dimension_world_acceptance_fixture","--",str(directory)],check=True);rows=json.loads((directory/"world-result.json").read_text());maximum=0
  for case,row in zip(cases,rows):
   frame=np.array(case["parent"]).reshape(4,4,order="F")@np.array(case["child"]).reshape(4,4,order="F");a,b=[vector(frame,case[key]) for key in ("a","b")];off=vector(frame,case["offset"],False)
   d=Dimension(V(*a),V(*b),V(*off),axis=case["axis"]);points=d.line_points()
   assert abs(d.value()-row["length"])<3e-5,(case,row,d.value())
   for expected,actual in zip(points,row["line_points"]):
    error=max(abs(x-y) for x,y in zip(coords(expected),actual));maximum=max(maximum,error);assert error<3e-5,(case,error)
   offset,axis=Dimension.placement_for_cursor(V(*a),V(*b),V(*vector(frame,case["cursor"])));assert axis==row["placement_axis"]
   assert max(abs(x-y) for x,y in zip(coords(offset),row["placement_offset"]))<3e-5
 report={"provenance":context(),"passed":True,"cases":len(cases),"maximum_world_line_coordinate_error":maximum,"scope":"Actual two-level GUI edit contexts with rotated/reflected/nonuniform parent-child transforms: world-axis x/y/z and aligned measurement, line endpoints and cursor placement against original Dimension APIs. Rendered labels remain separate."}
 Path("docs/audits/python-dimension-world-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
