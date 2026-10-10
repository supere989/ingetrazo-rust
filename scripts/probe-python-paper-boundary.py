#!/usr/bin/env python3
"""Observe unchanged original Qt WORLD mesh and HLR camera precision boundaries."""
import os, sys, json, hashlib
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
sys.path.insert(0, str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import qVersion
from PySide6.QtGui import QVector3D
from core.scene import Scene
from core.camera import OrbitCamera
from core.group import world_mesh, iter_placements, np_affine
import numpy as np
from PySide6.QtGui import QMatrix4x4
from core.hlr import camera_basis
from formats import igz
app = QApplication([])
root = Path(os.environ.get("INGETRAZO_VIEW_PDF_OUT", "docs/audits/sheet-view-vector-pdf"))
def xyz(v): return [v.x(), v.y(), v.z()]
def observe_group(g):
    m = world_mesh(g)
    return {"vertices": [xyz(v.position) for v in m.vertices],
            "edges": [[xyz(e.v0.position),xyz(e.v1.position),e.soft] for e in m.edges],
            "children": [observe_group(c) for c in g.children]}
def placements(g):
    rows=[]
    for item,m in iter_placements(g):
        m=m if m is not None else QMatrix4x4()
        points=[xyz(v.position) for v in item.mesh.vertices]
        rot,trans=np_affine(m)
        bulk=(np.asarray(points,dtype=np.float64).reshape((-1,3)) @ rot.T+trans).astype(np.float32).astype(np.float64).tolist()
        rows.append({"matrix":m.data(),"points":points,"walk":[xyz(m.map(v.position)) for v in item.mesh.vertices],"bulk":bulk})
    return rows
rows=[]
for path in sorted((root/"corpus").glob("*.reference.json")):
    ref=json.loads(path.read_text()); scene=Scene(); igz.load_into(scene,root/"corpus"/(ref["name"]+".igz")); camera=OrbitCamera()
    for field,value in ref["camera"].items(): setattr(camera,field,QVector3D(*value) if field in ("target","up") else value)
    basis=camera_basis(camera)
    rows.append({"name":ref["name"],"basis":[v.tolist() for v in basis],"eye":xyz(camera.eye()),"up":xyz(camera.up_vector()),"loose_vertices":[xyz(v.position) for v in scene.mesh.vertices],"groups":[observe_group(g) for g in scene.groups],"placements":[p for g in scene.groups for p in placements(g)]})
(root/"original-paper-boundary.json").write_text(json.dumps({"qt_version":qVersion(),"source_sha256":{name:hashlib.sha256((Path("ingetrazo")/name).read_bytes()).hexdigest() for name in ("core/camera.py","core/group.py","core/hlr.py")},"rows":rows},indent=2))
print("PASS actual original Qt paper boundary observations",len(rows))
