#!/usr/bin/env python3
"""Original OpenGL stencil capture fixture; run only on isolated X11."""
import sys,time,json,importlib.util
from pathlib import Path
from acceptance_context import context
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QSurfaceFormat,QVector3D
from PySide6.QtWidgets import QApplication
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setDepthBufferSize(24);fmt.setStencilBufferSize(8);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from views.viewport import Viewport
from core.scene import Scene
from core.section import SectionPlane
from core.style import Style
from core.orient import orient_outward
from core.mesh import Mesh
spec=importlib.util.spec_from_file_location("reference_orient",Path("ingetrazo/tests/test_orient.py"));ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)
root=Path(sys.argv[1]);root.mkdir(exist_ok=True);rows=[]
for name in ["closed","shared-slab","shared-slab-reversed","transparent","duplicate"]:
 vp=Viewport();vp.resize(800,600);m=Mesh();ref._two_storey_box(m)
 if name in ("closed","transparent","duplicate"):m.remove_face(m.faces[1])
 orient_outward(m)
 if name=="shared-slab-reversed":
  slab=next(f for f in m.faces if f.interior);slab.vertices.reverse()
 if name=="transparent":
  for f in m.faces:f.attrs["opacity"]=.5
 if name=="duplicate":
  original=list(m.faces)
  for f in original:m.add_face([QVector3D(p) for p in f.vertices])
 vp.scene=Scene(mesh=m);vp.scene.section_planes=[SectionPlane(QVector3D(0,0,1.5),QVector3D(0,0,1),active=True)];vp.scene.show_section_planes=False;vp.scene.show_section_cuts=True;vp.style_override=Style(face_mode="shaded",sky=False,edges=False,profiles=False,section_fill_color=(1.,0.,1.));vp.camera.set_view("top");vp.camera.perspective=False;vp.camera.target=QVector3D(.5,.5,1.5);vp.camera.distance=5.;vp.show()
 for _ in range(40):app.processEvents();time.sleep(.025)
 vp.update()
 for _ in range(10):app.processEvents();time.sleep(.025)
 image=vp.grabFramebuffer();assert not image.isNull();image.save(str(root/(name+".png")))
 count=0
 for y in range(image.height()):
  for x in range(image.width()):
   c=image.pixelColor(x,y)
   count+=c.red()>180 and c.blue()>180 and c.green()<80
 rows.append({"case":name,"magenta_fill_pixels":count,"faces":len(m.faces),"interior_flags":sum(f.interior for f in m.faces),"context_stencil_bits":vp.context().format().stencilBufferSize(),"width":image.width(),"height":image.height()});vp.close();vp.deleteLater();app.processEvents()
assert rows[0]["magenta_fill_pixels"]>2000,"Closed control must show a real cap"
assert all(r["context_stencil_bits"]>=8 for r in rows)
assert rows[1]["magenta_fill_pixels"]==rows[2]["magenta_fill_pixels"]==rows[3]["magenta_fill_pixels"]==0
assert rows[4]["magenta_fill_pixels"]>2000
report={"provenance":context(),"cases":rows,"scope":"Original Qt OpenGL on Linux X11 Mesa; shared interior slab suppresses cap although closed control fills. A physical correction that excludes the slab is an intentional divergence. These are literal pixel classification controls, not broad rendering parity."}
(root/"original-gl-results.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
