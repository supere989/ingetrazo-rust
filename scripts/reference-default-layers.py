#!/usr/bin/env python3
"""Frozen native document default registry and command reference controls."""
import copy,json,os,sys,zipfile,tempfile
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtGui import QVector3D as V
from core.scene import Scene
from core.layers import Layer,layer_of
from core.history import History,AssignLayerCommand
from core.group import Group
from core.mesh import Mesh
from formats.igz import save_scene,load_into
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
s=Scene();save_scene(s,out/"base.igz")
base=json.loads((out/"base.igz").read_text())
def state(s):return {"names":[l.name for l in s.layers],"default_visible":s.entity_visible(s.mesh.edges[0])if s.mesh.edges else s._layer_state(Group())[0]}
cases=[]
variants=[("omitted",None),("empty",[]),("authored-Layer0",[{"name":"Layer0","visible":False}]),("authored-Untagged",[{"name":"Untagged"}]),("explicit-default",[{"name":"Layer 0","visible":False}]),("authored-first",[{"name":"Layer0","visible":False},{"name":"Layer 0"}]),("missing-name",[{}])]
for name,layers in variants:
 d=copy.deepcopy(base);d["igz_format"]=2
 if layers is None:d["scene"].pop("layers",None)
 else:d["scene"]["layers"]=layers
 with zipfile.ZipFile(out/(name+".igz"),"w")as z:z.writestr("document.json",json.dumps(d))
 s=Scene();load_into(s,out/(name+".igz"));cases.append({"name":name,**state(s)})
s=Scene();s.layers.extend([Layer("Layer0"),Layer("Untagged")]);f=s.mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)]);e=s.mesh.edges[0];g=Group(Mesh(),name="tagged");s.groups.append(g);h=History(s)
def assignment():return {"layers":[l.name for l in s.layers],"face":f.attrs.get("layer"),"edge":e.layer,"group":g.layer}
steps=[assignment()]
with tempfile.TemporaryDirectory(prefix="ai-layer-reference-")as tmp:
 h.error_log=str(Path(tmp)/"errors.log")
 h.execute(AssignLayerCommand([f,e,g],"Layer0"));steps.append(assignment())
 h.execute(AssignLayerCommand([f,e,g],"Layer 0"));steps.append(assignment())
 h.undo();steps.append(assignment());h.redo();steps.append(assignment())
print(json.dumps({"new_scene":state(Scene()),"cases":cases,"assignment":steps},indent=2))
