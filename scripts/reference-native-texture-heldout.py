#!/usr/bin/env python3
"""Actual original viewport textures; literal inputs and framebuffer samples."""
import sys,time,json,hashlib,os
from pathlib import Path
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QSurfaceFormat,QVector3D,QImage,QColor
from PySide6.QtWidgets import QApplication
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setDepthBufferSize(24);fmt.setStencilBufferSize(8);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from views.viewport import Viewport
from core.mesh import Mesh
from core.scene import Scene
from core.style import Style
from formats.igz import save_scene
root=Path(sys.argv[1]).resolve();root.mkdir(parents=True,exist_ok=True)
for name,colors in {"quadrants":[(255,0,0,255),(0,255,0,255),(0,0,255,255),(255,255,255,255)],"alpha":[(37,93,157,128)]*4,"cutout":[(255,0,0,255),(0,255,0,0),(0,0,255,0),(255,255,255,255)],"back":[(0,0,255,255)]*4}.items():
 image=QImage(2,2,QImage.Format_RGBA8888)
 for i,color in enumerate(colors):image.setPixelColor(i%2,i//2,QColor(*color))
 assert image.save(str(root/(name+".png")))
from PySide6.QtGui import QMatrix4x4
from core.group import Group
rows=[]
cases=[
 ("tex-back-color-front",{"back_color":True}),
 ("tex-back-color-reversed",{"back_color":True,"reverse":True}),
 ("tex-trans-back-color-front",{"back_color":True,"opacity":.5}),
 ("tex-trans-back-color-reversed",{"back_color":True,"opacity":.5,"reverse":True}),
 ("tex-back-opaque-color-reversed",{"back_color":True,"back_opacity":1.,"reverse":True}),
 ("color-back-tex-reversed",{"color_front":True,"back_texture":True,"back_opacity":.3,"reverse":True}),
 ("color-back-tex-front",{"color_front":True,"back_texture":True,"back_opacity":.3}),
 ("tex-hole",{"hole":True}),
 ("tex-group-reflected",{"group":"reflect"}),
 ("tex-group-slanted",{"group":"slant"}),
 ("tex-mixed-order-tex-first",{"mixed":True,"opacity":.5}),
 ("tex-mixed-order-color-first",{"mixed":True,"opacity":.5,"color_first":True}),
 ("tex-mixed-backcolor-phase",{"mixed":True,"opacity":.5,"mixed_back":True}),
 ("tex-shaded-average",{"style":"shaded"}),
 ("tex-xray",{"style":"xray"}),
]
if "--shaded-translucent" in sys.argv:
 cases=[
  ("shaded-opaque",{"style":"shaded"}),
  ("shaded-trans-front",{"style":"shaded","opacity":.5}),
  ("shaded-trans-front-cull",{"style":"shaded","opacity":.5,"back_color":True}),
  ("shaded-opaque-back",{"style":"shaded","color_front":True,"back_texture":True,"back_quadrants":True,"back_opacity":1.,"reverse":True}),
  ("shaded-trans-back",{"style":"shaded","color_front":True,"back_texture":True,"back_quadrants":True,"back_opacity":.3,"reverse":True}),
  ("shaded-mixed-asset",{"style":"shaded","opacity":.5,"opaque_copy":True}),
 ]
for name,options in cases:
 mesh=Mesh();ring=[QVector3D(-1,-1,0),QVector3D(1,-1,0),QVector3D(1,1,0),QVector3D(-1,1,0)]
 if options.get("reverse"):ring.reverse()
 def add_color():
  color_ring=[p+QVector3D(0,0,.2) for p in ring]
  if options.get("mixed_back"):color_ring.reverse()
  face=mesh.add_face(color_ring);face.attrs.update(color=[1.,0.,0.],opacity=0. if options.get("mixed_back") else .5)
  if options.get("mixed_back"):face.attrs["back"]={"color":[0.,0.,1.],"opacity":.3}
 if options.get("color_first"):add_color()
 holes=[[QVector3D(-.25,-.25,0),QVector3D(-.25,.25,0),QVector3D(.25,.25,0),QVector3D(.25,-.25,0)]] if options.get("hole") else None
 face=mesh.add_face(ring,holes);face.attrs["opacity"]=options.get("opacity",1.)
 if options.get("color_front"):face.attrs["color"]=[1.,0.,0.]
 else:face.attrs["texture"]={"path":str(root/(("alpha" if options.get("mixed") else "quadrants")+".png")),"uvw":[.5,0,0,.5,0,.5,0,.5]}
 if options.get("back_color"):face.attrs["back"]={"color":[0.,0.,1.],"opacity":options.get("back_opacity",.3)}
 if options.get("back_texture"):face.attrs["back"]={"texture":{"path":str(root/("quadrants.png" if options.get("back_quadrants") else "back.png")),"uvw":[.5,0,0,.5,0,.5,0,.5]},"opacity":options.get("back_opacity",1.)}
 if options.get("opaque_copy"):
  opaque=mesh.add_face([p+QVector3D(0,0,-.2) for p in ring]);opaque.attrs["texture"]=dict(face.attrs["texture"])
 if options.get("mixed") and not options.get("color_first"):add_color()
 groups=[]
 if options.get("group"):
  group=Group(mesh,"Texture prototype");group.component=True;group.xform=QMatrix4x4()
  if options["group"]=="reflect":group.xform.scale(-1.5,1.,1.);group.xform.rotate(20.,0.,0.,1.)
  else:group.xform.rotate(45.,1.,0.,0.)
  groups=[group];mesh=Mesh()
 mode=options.get("style","textures")
 vp=Viewport();vp.resize(800,600);vp.scene=Scene(mesh=mesh,groups=groups);vp.style_override=Style(face_mode=mode,sky=False,edges=False,profiles=False,background=(1.,1.,1.));vp.camera.set_view("top");vp.camera.perspective=False;vp.camera.target=QVector3D();vp.camera.distance=4.;vp.setWindowTitle("IngeTrazo owned NVIDIA texture reference");vp.show()
 for _ in range(45):app.processEvents();time.sleep(.02)
 image=vp.grabFramebuffer();
 if os.environ.get("INGETRAZO_REQUIRE_NVIDIA")=="1":assert "NVIDIA GeForce RTX 5090" in vp.gl_info["renderer"],vp.gl_info
 assert not image.isNull();image.save(str(root/(name+".png")));save_scene(vp.scene,root/(name+".igz"))
 points=[(340,240),(460,240),(340,360),(460,360),(400,300)];samples=[]
 for x,y in points:c=image.pixelColor(x,y);samples.append([c.red(),c.green(),c.blue(),c.alpha()])
 rows.append({"case":name,"background":[1,1,1],"points":points,"samples":samples,"style_mode":mode,"show_textures":mode=="textures","gl_info":vp.gl_info,"native_input_sha256":hashlib.sha256((root/(name+".igz")).read_bytes()).hexdigest()})
 vp.close();vp.deleteLater();app.processEvents()
report={"cases":rows,"scope":"Actual original fifteen texture heldout cases; colored back, holes, transformed groups, mixed order, texture-average Shaded and XRay. All failures retained."};(root/"original-alpha-results.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
