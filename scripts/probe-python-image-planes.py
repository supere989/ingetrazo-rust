#!/usr/bin/env python3
"""Operative pinned History/context and IGZ image-plane contracts."""
import os,sys,json
from pathlib import Path
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
os.environ["XDG_CONFIG_HOME"]=str(out/"config");os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M,QImage,QColor
from views.viewport import Viewport
from core.image_plane import ImagePlane
from core.group import Group
from core.mesh import Mesh
from core.history import AddImagePlaneCommand,DeleteImagePlanesCommand,MoveImagePlanesCommand,SetImagePlaneOpacityCommand,MoveVerticesCommand
from formats import igz
from acceptance_context import context
app=QApplication([]);image=QImage(3,2,QImage.Format_RGBA8888);image.fill(QColor(13,79,151));image.save(str(out/"scan.png"))
rows={}
for nested,folded in [(False,False),(False,True),(True,False),(True,True)]:
 v=Viewport();v.flash_status=lambda *a:None
 im=ImagePlane(str(out/"scan.png"),V(2,3,4),V(5,0,0),V(0,0,3),.6,"Survey",.75,None,True)
 v.scene.image_planes=[im]
 mesh=Mesh();mesh.add_face([V(),V(1,0,0),V(0,1,0)]);g=Group(mesh,"Leaf");g.component=True;g.xform=M();g.xform.translate(10,0,0)
 if nested:
  outer=Group(name="Outer");outer.children=[g];outer.xform=M();v.scene.groups=[outer]
 else:v.scene.groups=[g]
 igz.save_scene(v.scene,out/(("nested" if nested else "direct")+"-input.igz"))
 if nested:v.begin_group_edit(outer)
 v.begin_group_edit(g)
 def state():
  planes=[x.to_dict()for x in v.scene.image_planes]
  for x in planes:x["texture"]["path"]="scan.png"
  return {"planes":planes,"layers":[x.name for x in v.scene.layers],"undo":len(v.history.undo_stack),"redo":len(v.history.redo_stack)}
 states=[state()]
 v.history.execute(MoveImagePlanesCommand([im],V(1,2,3)))
 v.history.execute(SetImagePlaneOpacityCommand(im,.375))
 new=ImagePlane(str(out/"scan.png"),V(8,9,10),V(2,0,0),V(0,1,0),.5,"New")
 v.history.execute(AddImagePlaneCommand(new));states.append(state())
 if folded:v.history.execute(MoveVerticesCommand([V(10,0,0)],V(2,0,0)))
 v.end_one_group_edit()
 if nested:v.end_one_group_edit()
 states.append(state())
 for _ in range(4 if folded else 3):v.history.undo();states.append(state())
 for _ in range(4 if folded else 3):v.history.redo();states.append(state())
 v.history.execute(DeleteImagePlanesCommand([im]));states.append(state());v.history.undo();states.append(state());v.history.redo();states.append(state())
 key=("nested_"if nested else "")+("folded"if folded else "ordinary");rows[key]=states
 igz.save_scene(v.scene,out/(key+".igz"))
Path(out/"original.json").write_text(json.dumps({"provenance":context(),"cases":rows},indent=2)+"\n")
