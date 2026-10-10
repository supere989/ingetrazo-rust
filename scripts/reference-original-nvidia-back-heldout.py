#!/usr/bin/env python3
"""Literal original colored alpha/depth/bucket order fixtures on isolated X11."""
import sys,time,json,hashlib,os
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
if os.environ.get("INGETRAZO_ALPHA_ROUNDING_CASES")=="1":
 cases.update({"rgb-tie-even":([([16.5/62.,32.5/62.,48.5/62.],.5,0.)],[0.,0.,0.]),"rgb-tie-odd":([([17.5/62.,33.5/62.,49.5/62.],.5,0.)],[0.,0.,0.]),"opacity-round-5005":([([1.,0.,0.],.5005,0.)],[0.,0.,0.]),"opacity-round-1255":([([1.,0.,0.],.1255,0.)],[0.,0.,0.]),"opacity-threshold-f64":([([1.,0.,0.],.99899999999,0.)],[0.,0.,0.])})
if os.environ.get("INGETRAZO_ALPHA_BACK_CASES")=="1":
 for front_op,back_op in [(0.5,0.3),(1.,0.3),(0.5,1.),(1.,1.)]:
  for reverse in [False,True]:
   cases[f"back-{front_op}-{back_op}-{'reversed' if reverse else 'front'}"]=([([1.,0.,0.],front_op,0.,{"color":[0.,0.,1.],"opacity":back_op},reverse)],[1.,1.,1.])
if os.environ.get("INGETRAZO_ALPHA_BACK_CASES")=="1":
  glass=([1.,0.,0.],1.,0.,{"color":[0.,0.,1.],"opacity":.3},False)
  cases["back-gate-activated-front"]=([glass,([0.,1.,0.],1.,0.,{},False,10.)],[1.,1.,1.])
  cases["back-gate-activated-reversed"]=([(*glass[:4],True),([0.,1.,0.],1.,0.,{},False,10.)],[1.,1.,1.])
  cases["back-order-translucent-opaque"]=([([1.,0.,0.],.5,0.,{"color":[0.,0.,1.],"opacity":.3},True),([1.,0.,0.],1.,.2,{"color":[0.,1.,0.],"opacity":.3},True)],[1.,1.,1.])
  cases["back-order-opaque-translucent"]=([([1.,0.,0.],1.,0.,{"color":[0.,0.,1.],"opacity":.3},True),([1.,0.,0.],.5,.2,{"color":[0.,1.,0.],"opacity":.3},True)],[1.,1.,1.])
cases={"back-slanted-reversed":([([1.,0.,0.],.5,0.,{"color":[0.,0.,1.],"opacity":.3},True)],[1.,1.,1.]),"back-holed-reversed":([([1.,0.,0.],.5,0.,{"color":[0.,0.,1.],"opacity":.3},True)],[1.,1.,1.]),"back-reflected-group":([([1.,0.,0.],.5,0.,{"color":[0.,0.,1.],"opacity":.3},False)],[1.,1.,1.]),"back-group-opaque-gate":([([1.,0.,0.],1.,0.,{"color":[0.,0.,1.],"opacity":.3},False)],[1.,1.,1.])}
for name,(faces,bg) in cases.items():
 vp=Viewport();vp.setWindowTitle("IngeTrazo original NVIDIA parity probe");vp.resize(800,600);mesh=Mesh()
 for spec in faces:
  color,opacity,z=spec[:3];ring=[QVector3D(-1,-1,z),QVector3D(1,-1,z),QVector3D(1,1,z),QVector3D(-1,1,z)]
  if len(spec)>5:ring=[p+QVector3D(spec[5],0,0) for p in ring]
  if name=="back-slanted-reversed":ring=[p+QVector3D(0,0,.25*p.x())for p in ring]
  if len(spec)>4 and spec[4]:ring.reverse()
  hole=[[QVector3D(-.25,-.25,z),QVector3D(-.25,.25,z),QVector3D(.25,.25,z),QVector3D(.25,-.25,z)]]if name=="back-holed-reversed"else None
  face=mesh.add_face(ring,hole_loops=hole);face.attrs.update(color=color,opacity=opacity)
  if len(spec)>3:face.attrs["back"]=spec[3]
 vp.scene=Scene(mesh=mesh)
 if name in ["back-reflected-group","back-group-opaque-gate"]:
  from core.group import Group
  from PySide6.QtGui import QMatrix4x4
  if name=="back-reflected-group":
   g=Group(mesh,name="mirror");g.component=False;g.xform=QMatrix4x4();g.xform.scale(-1.,1.,1.);vp.scene.mesh=Mesh();vp.scene.groups=[g]
  else:
   g=Group(name="opaque-offscreen");face=g.mesh.add_face([QVector3D(9,-1,0),QVector3D(11,-1,0),QVector3D(11,1,0),QVector3D(9,1,0)]);face.attrs.update(color=[0.,1.,0.],opacity=1.);g.component=False;vp.scene.groups=[g]
 if name=="section-clipped":vp.scene.section_planes=[SectionPlane(QVector3D(0,0,.1),QVector3D(0,0,1),active=True)];vp.scene.show_section_cuts=True;vp.scene.show_section_planes=False
 vp.style_override=Style(face_mode="shaded",sky=False,edges=False,profiles=False,background=tuple(bg));vp.camera.set_view("top");vp.camera.perspective=False;vp.camera.target=QVector3D(0,0,0);vp.camera.distance=4.;vp.show()
 for _ in range(45):app.processEvents();time.sleep(.02)
 image=vp.grabFramebuffer();assert not image.isNull();image.save(str(root/(name+".png")));save_scene(vp.scene,root/(name+".igz"));assert "NVIDIA" in vp.gl_info["renderer"],vp.gl_info;pixel=image.pixelColor(400,300)
 rows.append({"case":name,"gl_info":vp.gl_info,"center_rgba":[pixel.red(),pixel.green(),pixel.blue(),pixel.alpha()],"background":bg,"faces":faces,"native_input_sha256":hashlib.sha256((root/(name+".igz")).read_bytes()).hexdigest(),"cached_opacity_runs":[list(run) for run in vp._tcol_runs],"ordinary_faces_count":vp._faces_count,"front_culled_opaque_run":list(vp._fvcol_run),"opaque_back_run":list(vp._back_vcol_run),"cached_back_opacity_runs":[list(run) for run in vp._back_tcol_runs],"qt_image_format":str(image.format())});vp.close();vp.deleteLater();app.processEvents()
report={"provenance":context(),"cases":rows,"scope":"Actual original Qt OpenGL encoded-RGB blend, depth-write-off, cached opacity run order; colored untextured front surfaces only. Not texture/back-side/instancing/Xray coverage."};(root/"original-alpha-results.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
