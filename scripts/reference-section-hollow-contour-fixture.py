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
from formats.igz import save_scene
from core.orient import orient_outward
from core.mesh import Mesh
spec=importlib.util.spec_from_file_location("reference_orient",Path("ingetrazo/tests/test_orient.py"));ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)
root=Path(sys.argv[1]);root.mkdir(exist_ok=True);rows=[]
for name in ["hollow","hollow-iso","transparent-hollow","hollow-seam"]:
 vp=Viewport();vp.resize(800,600);m=Mesh()
 outer=lambda z:[QVector3D(0,0,z),QVector3D(1,0,z),QVector3D(1,1,z),QVector3D(0,1,z)]
 inner=lambda z:[QVector3D(.25,.25,z),QVector3D(.75,.25,z),QVector3D(.75,.75,z),QVector3D(.25,.75,z)]
 m.add_face(list(reversed(outer(0))),[inner(0)])
 m.add_face(outer(2),[list(reversed(inner(2)))])
 zs=[0,1.5,2] if name=="hollow-seam" else [0,2]
 for za,zb in zip(zs,zs[1:]):
  for ring,reverse in [(outer,False),(inner,True)]:
   for i in range(4):
    j=(i+1)%4;points=[ring(za)[i],ring(za)[j],ring(zb)[j],ring(zb)[i]]
    m.add_face(list(reversed(points)) if reverse else points)
 orient_outward(m)
 if name=="transparent-hollow":
  for f in m.faces:f.attrs["opacity"]=.5
 vp.scene=Scene(mesh=m);vp.scene.section_planes=[SectionPlane(QVector3D(0,0,1.5),QVector3D(0,0,1),active=True)];vp.scene.show_section_planes=False;vp.scene.show_section_cuts=True;vp.style_override=Style(face_mode="shaded",sky=False,edges=False,profiles=False,section_fill_color=(1.,0.,1.));vp.camera.set_view("iso" if name.endswith("iso") else "front" if name.endswith("front") else "top");vp.camera.perspective=False;vp.camera.target=QVector3D(.5,.5,1.5);vp.camera.distance=5.;vp.show()
 for _ in range(40):app.processEvents();time.sleep(.025)
 vp.update()
 for _ in range(10):app.processEvents();time.sleep(.025)
 image=vp.grabFramebuffer();assert not image.isNull();image.save(str(root/(name+".png")))
 save_scene(vp.scene,root/(name+".igz"))
 count=0
 for y in range(image.height()):
  for x in range(image.width()):
   c=image.pixelColor(x,y)
   count+=c.red()>180 and c.blue()>180 and c.green()<80
 rows.append({"case":name,"magenta_fill_pixels":count,"faces":len(m.faces),"interior_flags":sum(f.interior for f in m.faces),"context_stencil_bits":vp.context().format().stencilBufferSize(),"width":image.width(),"height":image.height()});vp.close();vp.deleteLater();app.processEvents()
assert rows[0]["magenta_fill_pixels"]>2000
assert rows[2]["magenta_fill_pixels"]==0
report={"provenance":context(),"cases":rows,"scope":"Original Qt OpenGL on Linux X11 Mesa; shared interior slab suppresses cap although closed control fills. A physical correction that excludes the slab is an intentional divergence. These are literal pixel classification controls, not broad rendering parity."}
(root/"original-gl-results.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
