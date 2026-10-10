#!/usr/bin/env python3
import os,sys,json,tempfile,subprocess,math
from pathlib import Path
import numpy as np
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.viewport import Viewport
from core.group import Group
from core.mesh import Mesh
from core.dimension import Dimension
def transform(angle,scale,translation,axis):
 a=math.radians(angle);c,s=math.cos(a),math.sin(a);r=np.eye(4)
 if axis=="z":r[:3,:3]=[[c,-s,0],[s,c,0],[0,0,1]]
 else:r[:3,:3]=[[1,0,0],[0,c,-s],[0,s,c]]
 result=r@np.diag([*scale,1]);result[:3,3]=translation;return result
def vector(matrix,v,point=True):return (matrix@np.array([*v,1 if point else 0]))[:3]
def coords(v):return [v.x(),v.y(),v.z()]
def observe_entry(case,d):
 vp=Viewport();vp.flash_status=lambda *args:None
 matrix=lambda key:M(*np.array(case[key]).reshape(4,4,order="F").flatten().tolist())
 leaf=Group(Mesh(),name="leaf");leaf.component=True;leaf.xform=matrix("child")
 outer=Group(Mesh(),name="outer");outer.component=True;outer.xform=matrix("parent");outer.children=[leaf]
 vp.scene.groups=[outer];vp.scene.dimensions=[d]
 snapshot=lambda:[coords(d.a),coords(d.b),coords(d.offset),d.value(),*[coords(p)for p in d.line_points()]]
 before=snapshot()
 vp.begin_group_edit(outer);parent_baked=outer.xform == M()
 vp.begin_group_edit(leaf);leaf_baked=leaf.xform is None
 assert parent_baked and leaf_baked,"Original source placement entry differs"
 assert vp.scene.dimensions[0] is d and snapshot()==before,"Original world annotation changed on edit entry"
 drawing=list(vp.scene.drawing_frame.data())
 vp.end_one_group_edit();vp.end_one_group_edit()
 assert snapshot()==before,"Original world annotation changed on edit exit"
 vp.deleteLater()
 return dict(world_annotation_preserved=True,parent_baked=parent_baked,leaf_baked=leaf_baked,drawing_frame=drawing)
def main():
 app=QApplication([]);observations=[]
 parent=transform(20,[1,.75,2],[10,20,30],"x");cases=[]
 for angle,scale in ((0,[1,1,1]),(30,[-2,3,.5]),(75,[.5,-2,4])):
  child=transform(angle,scale,[1,2,3],"z")
  for axis in (None,"x","y","z"):
   cases.append({"parent":parent.flatten(order="F").tolist(),"child":child.flatten(order="F").tolist(),"a":[1,2,3],"b":[4,6,5],"offset":[-2,3,4],"cursor":[8,-2,4],"axis":axis})
 with tempfile.TemporaryDirectory() as tmp:
  directory=Path(tmp);(directory/"world-cases.json").write_text(json.dumps(cases));subprocess.run(([str(Path(sys.argv[1]).resolve()),str(directory)] if len(sys.argv)>1 else ["cargo","run","--locked","-p","ingetrazo-gui","--example","dimension_world_acceptance_fixture","--",str(directory)]),check=True);rows=json.loads((directory/"world-result.json").read_text());maximum=0
  assert len(rows)==len(cases)==12,"Dimension case inventory mismatch"
  for case,row in zip(cases,rows):
   frame=np.array(case["parent"]).reshape(4,4,order="F")@np.array(case["child"]).reshape(4,4,order="F");a,b=[vector(frame,case[key]) for key in ("a","b")];off=vector(frame,case["offset"],False)
   d=Dimension(V(*a),V(*b),V(*off),axis=case["axis"]);observations.append(observe_entry(case,d));assert row["world_annotations_preserved"];points=d.line_points();assert len(row["line_points"])==2 and all(len(p)==3 for p in row["line_points"])
   assert abs(d.value()-row["length"])<3e-5,(case,row,d.value())
   for expected,actual in zip(points,row["line_points"]):
    error=max(abs(x-y) for x,y in zip(coords(expected),actual));maximum=max(maximum,error);assert error<3e-5,(case,error)
   offset,axis=Dimension.placement_for_cursor(V(*a),V(*b),V(*vector(frame,case["cursor"])));assert axis==row["placement_axis"]
   assert max(abs(x-y) for x,y in zip(coords(offset),row["placement_offset"]))<3e-5
 report={"provenance":context(),"passed":True,"cases":len(cases),"maximum_world_line_coordinate_error":maximum,"source_edit_entry_observations":observations,"scope":"Actual two-level GUI edit contexts with rotated/reflected/nonuniform parent-child transforms: world-axis x/y/z and aligned measurement, line endpoints and cursor placement against original Dimension APIs. Rendered labels remain separate."}
 Path("docs/audits/python-dimension-world-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
