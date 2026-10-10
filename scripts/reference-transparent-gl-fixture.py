#!/usr/bin/env python3
"""Literal original colored alpha/depth/bucket order fixtures on isolated X11."""
import sys,time,json,hashlib
from pathlib import Path
from acceptance_context import context
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QSurfaceFormat,QVector3D
from PySide6.QtWidgets import QApplication
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setDepthBufferSize(24);fmt.setStencilBufferSize(8);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from views.viewport import Viewport
from core.scene import Scene
from core.mesh import Mesh
from core.style import Style
from core.section import SectionPlane
from formats.igz import save_scene
root=Path(sys.argv[1]);root.mkdir(parents=True,exist_ok=True);rows=[]
red=([1.,0.,0.],.5,0.);blue=([0.,0.,1.],.3,.2)
cases={"red-half-white":([red],[1.,1.,1.]),"red-half-black":([red],[0.,0.,0.]),"opacity-bucket-order":([red,blue],[1.,1.,1.]),"opaque-front":([red,blue,([0.,1.,0.],1.,.3)],[1.,1.,1.]),"opaque-back":([red,blue,([0.,1.,0.],1.,-.3)],[1.,1.,1.]),"equal-opacity-forward":([red,([0.,0.,1.],.5,.2)],[1.,1.,1.]),"equal-opacity-reversed":([([0.,0.,1.],.5,.2),red],[1.,1.,1.]),"opacity-995":([([1.,0.,0.],.995,0.)],[1.,1.,1.]),"opacity-zero":([([1.,0.,0.],0.,0.)],[1.,1.,1.]),"section-clipped":([red,blue],[1.,1.,1.])}
for name,(faces,bg) in cases.items():
 vp=Viewport();vp.resize(800,600);mesh=Mesh()
 for color,opacity,z in faces:
  face=mesh.add_face([QVector3D(-1,-1,z),QVector3D(1,-1,z),QVector3D(1,1,z),QVector3D(-1,1,z)]);face.attrs.update(color=color,opacity=opacity)
 vp.scene=Scene(mesh=mesh)
 if name=="section-clipped":vp.scene.section_planes=[SectionPlane(QVector3D(0,0,.1),QVector3D(0,0,1),active=True)];vp.scene.show_section_cuts=True;vp.scene.show_section_planes=False
 vp.style_override=Style(face_mode="shaded",sky=False,edges=False,profiles=False,background=tuple(bg));vp.camera.set_view("top");vp.camera.perspective=False;vp.camera.target=QVector3D(0,0,0);vp.camera.distance=4.;vp.show()
 for _ in range(45):app.processEvents();time.sleep(.02)
 image=vp.grabFramebuffer();assert not image.isNull();image.save(str(root/(name+".png")));save_scene(vp.scene,root/(name+".igz"));pixel=image.pixelColor(400,300)
 rows.append({"case":name,"center_rgba":[pixel.red(),pixel.green(),pixel.blue(),pixel.alpha()],"background":bg,"faces":faces,"native_input_sha256":hashlib.sha256((root/(name+".igz")).read_bytes()).hexdigest(),"cached_opacity_runs":[list(run) for run in vp._tcol_runs],"qt_image_format":str(image.format())});vp.close();vp.deleteLater();app.processEvents()
report={"provenance":context(),"cases":rows,"scope":"Actual original Qt OpenGL encoded-RGB blend, depth-write-off, cached opacity run order; colored untextured front surfaces only. Not texture/back-side/instancing/Xray coverage."};(root/"original-alpha-results.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
