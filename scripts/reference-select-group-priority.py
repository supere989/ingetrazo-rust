#!/usr/bin/env python3
import os,sys,json,importlib.util
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtCore import QPointF,Qt
from tools.select import SelectTool
from tools.base import ToolContext
spec=importlib.util.spec_from_file_location("fixtures","/work/ingetrazo/tests/test_hidden_things_are_clickable.py");f=importlib.util.module_from_spec(spec);spec.loader.exec_module(f)
vp=f._vp();g=f._boxed_group(vp);face=vp.scene.mesh.add_face([f.V(0,0,1),f.V(4,0,1),f.V(4,4,1),f.V(0,4,1)]);vp.scene.version+=1
point=f.V(2,2,0);px=vp._world_to_pixel(point)
assert vp.pick_face(*px) is face and vp.pick_group(*px) is g
SelectTool().on_click(ToolContext(vp,point,QPointF(*px),Qt.NoModifier,None))
assert vp.scene.selection=={g}
from pathlib import Path
Path(sys.argv[1]).write_text(json.dumps({"matrix":list((vp.camera.projection_matrix()*vp.camera.view_matrix()).data()),"eye":[vp.camera.eye().x(),vp.camera.eye().y(),vp.camera.eye().z()],"pixel":list(px),"point":[2.,2.,0.],"selection":[["group",0]],"source_commit":"6be29fe437a7cd992d4e079f71720821b25216b9"},indent=2)+"\n")
print("PASS actual source overlapping loose-face/group priority")
