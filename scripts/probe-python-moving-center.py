#!/usr/bin/env python3
"""Pinned original moving-curve center exclusion controller recipe, not GUI proof."""
import os,sys,math,json
os.environ["QT_QPA_PLATFORM"]="offscreen"
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from PySide6.QtCore import QPointF
from types import SimpleNamespace as N
from views.viewport import Viewport
from tools.move import MoveTool
from core.snap import compute_snap
app=QApplication([]);vp=Viewport();vp.resize(1000,700)
c=V(2,3,1);points=[c+V(1.2*math.cos(i*math.tau/24),1.2*math.sin(i*math.tau/24),0) for i in range(24)]
for i,p in enumerate(points):vp.scene.mesh.add_edge(p,points[(i+1)%24]).curve=71
vp.camera.target=V(2,3,0);vp.camera.distance=10;vp.camera.pitch=math.pi/6;vp.camera.yaw=-math.pi/2;vp.camera.perspective=True
vp.scene.select(vp.scene.mesh.edges);tool=MoveTool();vp.set_active_tool(tool);tool.on_click(N(viewport=vp,world=points[0],screen=QPointF(0,0)))
vp._hover_edge=vp.scene.mesh.edges[0]
pixel=vp._world_to_pixel(points[0]);found=vp._update_center_ref(*pixel);assert found is not None
snapscene=vp._snap_scene(*pixel);excluded=tool.snap_excluded()[0]
assert len(excluded)==24
assert all(id(e)not in excluded for e in snapscene.edges)
centers=[e for e in snapscene.edges if getattr(e,"center",False)];assert len(centers)==1
r=compute_snap(c,vp._world_to_pixel(c),snapscene,vp._world_to_pixel,9)
assert r.kind=="center",r.kind
from acceptance_context import context
report={"provenance":context(),"excluded_edges":len(excluded),"remaining_edges":len(snapscene.edges),"center_pseudoedges":len(centers),"kind":r.kind,"point":[r.point.x(),r.point.y(),r.point.z()],"scope":"Actual original Move.on_click/snap_excluded and Viewport center acquisition/snap-scene assembly, controlled hovered edge; no actual pointer or timer acceptance"}
from pathlib import Path
Path("docs/audits/moving-center-source-policy.json").write_text(json.dumps(report,indent=2)+"\n")
print("Original moving-curve excluded edges retain center: PASS")
