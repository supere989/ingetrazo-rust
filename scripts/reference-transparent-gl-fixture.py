#!/usr/bin/env python3
"""Literal original colored alpha/depth/bucket order fixtures on isolated X11."""
import sys,time,json,hashlib,os
from pathlib import Path
from acceptance_context import context
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QSurfaceFormat,QVector3D,QMatrix4x4
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
if os.environ.get("INGETRAZO_ALPHA_INSTANCED_GATE")=="1":
 glass=([1.,0.,0.],1.,0.,{"color":[0.,0.,1.],"opacity":.3},False)
 cases["back-gate-instanced-only-front"]=([glass,([0.,1.,0.],1.,0.,{},False,10.)],[1.,1.,1.])
 cases["back-gate-instanced-only-reversed"]=([(*glass[:4],True),([0.,1.,0.],1.,0.,{},False,10.)],[1.,1.,1.])
if os.environ.get("INGETRAZO_ALPHA_GROUP_SHADE")=="1":
 for name in ["group-rotated-prototype","group-nested-rotated-prototype","group-classic-baked-rotation"]:cases[name]=([red],[1.,1.,1.])
for name,(faces,bg) in cases.items():
 vp=Viewport();vp.resize(800,600);mesh=Mesh()
 for spec in faces:
  color,opacity,z=spec[:3];ring=[QVector3D(-1,-1,z),QVector3D(1,-1,z),QVector3D(1,1,z),QVector3D(-1,1,z)]
  if len(spec)>5:ring=[p+QVector3D(spec[5],0,0) for p in ring]
  if len(spec)>4 and spec[4]:ring.reverse()
  face=mesh.add_face(ring);face.attrs.update(color=color,opacity=opacity)
  if len(spec)>3:face.attrs["back"]=spec[3]
 groups=[]
 if name.startswith("back-gate-instanced-only"):
  from core.group import Group
  f=mesh.faces[-1];group_mesh=Mesh();gface=group_mesh.add_face(f.vertices);gface.attrs.update(f.attrs);mesh.remove_face(f)
  group=Group(group_mesh,"Opaque activator instance");group.component=True;group.xform=QMatrix4x4();groups=[group]
 if name.startswith("group-"):
  from core.group import Group
  group=Group(mesh,"Shading prototype");xf=QMatrix4x4();xf.rotate(-45.,0.,1.,0.)
  if name=="group-classic-baked-rotation":
   from core.group import transformed_mesh
   group.mesh=transformed_mesh(mesh,xf)
  elif name=="group-nested-rotated-prototype":
   group.xform=xf;parent=Group(name="Parent");parent.adopt([group]);parent.xform.rotate(20.,0.,0.,1.);group=parent
  else:group.xform=xf
  groups=[group];mesh=Mesh()
 vp.scene=Scene(mesh=mesh,groups=groups)
 if name=="section-clipped":vp.scene.section_planes=[SectionPlane(QVector3D(0,0,.1),QVector3D(0,0,1),active=True)];vp.scene.show_section_cuts=True;vp.scene.show_section_planes=False
 vp.style_override=Style(face_mode="shaded",sky=False,edges=False,profiles=False,background=tuple(bg));vp.camera.set_view("top");vp.camera.perspective=False;vp.camera.target=QVector3D(0,0,0);vp.camera.distance=4.;vp.show()
 for _ in range(45):app.processEvents();time.sleep(.02)
 image=vp.grabFramebuffer();assert not image.isNull();image.save(str(root/(name+".png")));save_scene(vp.scene,root/(name+".igz"));pixel=image.pixelColor(400,300)
 rows.append({"case":name,"center_rgba":[pixel.red(),pixel.green(),pixel.blue(),pixel.alpha()],"background":bg,"faces":faces,"native_input_sha256":hashlib.sha256((root/(name+".igz")).read_bytes()).hexdigest(),"cached_opacity_runs":[list(run) for run in vp._tcol_runs],"ordinary_faces_count":vp._faces_count,"front_culled_opaque_run":list(vp._fvcol_run),"opaque_back_run":list(vp._back_vcol_run),"cached_back_opacity_runs":[list(run) for run in vp._back_tcol_runs],"qt_image_format":str(image.format())});vp.close();vp.deleteLater();app.processEvents()
report={"provenance":context(),"cases":rows,"scope":"Actual original Qt OpenGL encoded-RGB blend, depth-write-off, cached opacity run order; colored untextured front surfaces only. Not texture/back-side/instancing/Xray coverage."};(root/"original-alpha-results.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
