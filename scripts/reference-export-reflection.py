#!/usr/bin/env python3
"""Pinned exporter for the identical nested reflected triangle used by CI."""
import sys,json,runpy
from pathlib import Path
sys.path[:0]=["ingetrazo","scripts"]
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from formats.gltf import save_glb
s=Scene();mesh=Mesh();mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)])
c=Group(mesh,"reflected child");c.xform=M();c.xform.translate(2,3,4);c.xform.scale(-1,1,1)
p=Group(Mesh(),"translated parent");p.xform=M();p.xform.translate(10,20,30);p.children=[c];s.groups=[p]
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True);save_glb(s,out/"original.glb")
d=runpy.run_path("scripts/check-gltf-rich.py")["decode"](out/"original.glb");(out/"original.json").write_text(json.dumps(d,indent=2)+"\n")
