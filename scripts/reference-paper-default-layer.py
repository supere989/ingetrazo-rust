#!/usr/bin/env python3
import json,os,sys
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from core.scene import Scene
from core.layers import Layer
from core.saved_views import SavedView
from core.camera import OrbitCamera
from PySide6.QtGui import QVector3D as V
cases=[]
for hidden in [[],["Layer0"],["Layer 0"],["Layer0","Layer 0"]]:
 s=Scene();s.layers.insert(0,Layer("Layer0"));f=s.mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)]);a=s.mesh.add_face([V(3,0,0),V(4,0,0),V(3,1,0)]);a.attrs["layer"]="Layer0";e=s.mesh.add_edge(V(10,0,0),V(11,0,0));SavedView("paper",hidden_layers=hidden).apply(s,OrbitCamera());cases.append({"hidden":hidden,"default_face":s.entity_visible(f),"authored_face":s.entity_visible(a),"loose_wire":s.entity_visible(e)})
print(json.dumps(cases,indent=2))
