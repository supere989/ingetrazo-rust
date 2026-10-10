#!/usr/bin/env python3
"""Actual original viewport textures; literal inputs and framebuffer samples."""
import sys,time,json,hashlib,os,ctypes
from pathlib import Path
sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtGui import QSurfaceFormat,QVector3D,QVector4D,QImage,QColor
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
from core.image_plane import ImagePlane
rows=[]
recipes=[("image-quadrants","quadrants",1.,False,"textures",True,None),
("image-alpha","alpha",.5,False,"textures",True,None),
("image-cutout","cutout",1.,False,"textures",True,None),
("image-opacity-0999","quadrants",.999,False,"textures",True,None),
("image-opacity-below","quadrants",.99899999999,False,"textures",True,None),
("image-reversed","quadrants",1.,True,"textures",True,None),
("image-wireframe","quadrants",1.,False,"wireframe",False,None),
("image-hidden-line","quadrants",1.,False,"hidden",False,None),
("image-xray","quadrants",1.,False,"xray",False,None),
("image-hidden-layer","quadrants",1.,False,"textures",True,"Hidden"),
("image-overlap","quadrants",.5,False,"textures",True,None),
("image-opaque-behind","quadrants",1.,False,"textures",True,None),
("image-faded-behind","quadrants",.5,False,"textures",True,None),
("image-threshold-behind","quadrants",.999,False,"textures",True,None),
("image-below-threshold-behind","quadrants",.99899999999,False,"textures",True,None),
("image-coplanar-trace","quadrants",1.,False,"textures",True,None)]
for name,texture,opacity,reverse,mode,show,layer in recipes:
 vp=Viewport();vp.resize(800,600);vp.scene=Scene(mesh=Mesh());
 if os.environ.get("INGETRAZO_IMAGE_OUTLINE_ABLATION")=="1":vp._draw_image_outlines=lambda painter:None
 if name.endswith("behind") or name=="image-coplanar-trace":
  z=0. if name=="image-coplanar-trace" else -.1
  face=vp.scene.mesh.add_face([QVector3D(-1,-1,z),QVector3D(1,-1,z),QVector3D(1,1,z),QVector3D(-1,1,z)]);face.attrs["color"]=(0.,0.,1.)
 im=ImagePlane(str(root/(texture+".png")),QVector3D(-1,-1,0) if not reverse else QVector3D(1,-1,0),QVector3D(2,0,0) if not reverse else QVector3D(-2,0,0),QVector3D(0,2,0));im.opacity=opacity;im.layer=layer;vp.scene.image_planes=[im]
 if layer:
  from core.layers import Layer
  vp.scene.layers.append(Layer(layer,visible=False))
 if name=="image-overlap":
  front=ImagePlane(str(root/"alpha.png"),QVector3D(-1,-1,.1),QVector3D(2,0,0),QVector3D(0,2,0));front.opacity=.5;vp.scene.image_planes.append(front)
 vp.style_override=Style(face_mode=mode,sky=False,edges=False,profiles=False,background=(1.,1.,1.));vp.camera.set_view("top");vp.camera.perspective=False;vp.camera.target=QVector3D();vp.camera.distance=4.;vp.setWindowTitle("IngeTrazo owned image-plane reference")
 if os.environ.get("INGETRAZO_IMAGE_PROJECTION_Y_ABLATION"):
  original_projection=vp.camera.projection_matrix
  coefficient=float(os.environ["INGETRAZO_IMAGE_PROJECTION_Y_ABLATION"])
  def diagnostic_projection(original_projection=original_projection,coefficient=coefficient):
   m=original_projection();r=m.row(1);m.setRow(1,QVector4D(r.x(),coefficient,r.z(),r.w()));return m
  vp.camera.projection_matrix=diagnostic_projection
 vp.show()
 for _ in range(45):app.processEvents();time.sleep(.02)
 image=vp.grabFramebuffer()
 vp.makeCurrent();gl=ctypes.CDLL("libGL.so.1");sample_count=ctypes.c_int();sample_buffers=ctypes.c_int();gl.glGetIntegerv(0x80A9,ctypes.byref(sample_count));gl.glGetIntegerv(0x80A8,ctypes.byref(sample_buffers));vp.doneCurrent()
 if os.environ.get("INGETRAZO_REQUIRE_NVIDIA")=="1":assert "NVIDIA GeForce RTX 5090" in vp.gl_info["renderer"],vp.gl_info
 assert not image.isNull();image.save(str(root/(name+".png")));save_scene(vp.scene,root/(name+".igz"))
 points=[(340,240),(460,240),(340,360),(460,360),(400,300)];samples=[]
 for x,y in points:c=image.pixelColor(x,y);samples.append([c.red(),c.green(),c.blue(),c.alpha()])
 rows.append({"case":name,"background":[1,1,1],"points":points,"samples":samples,"gl_info":vp.gl_info,"actual_gl_samples":sample_count.value,"actual_gl_sample_buffers":sample_buffers.value,"qt_surface_samples":vp.format().samples(),"projection_y_ablation":os.environ.get("INGETRAZO_IMAGE_PROJECTION_Y_ABLATION"),"image_outline_ablation":os.environ.get("INGETRAZO_IMAGE_OUTLINE_ABLATION")=="1","projection_matrix":list(vp.camera.projection_matrix().data()),"view_matrix":list(vp.camera.view_matrix().data()),"mvp":list((vp.camera.projection_matrix()*vp.camera.view_matrix()).data()),"style_mode":{"wireframe":"wireframe","hidden":"hidden_line","xray":"xray"}.get(mode,"textures"),"show_textures":show,"native_input_sha256":hashlib.sha256((root/(name+".igz")).read_bytes()).hexdigest()})
 vp.close();vp.deleteLater();app.processEvents()
report={"cases":rows,"scope":"Actual original sixteen image-plane controls; no full GPU image parity claim"};(root/"original-alpha-results.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
