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
for name,colors in {"quadrants":[(255,0,0,255),(0,255,0,255),(0,0,255,255),(255,255,255,255)],"alpha":[(37,93,157,128)]*4,"cutout":[(255,0,0,255),(0,255,0,0),(0,0,255,0),(255,255,255,255)],"tinted":[(37,93,157,64),(251,173,3,129),(9,231,49,191),(145,31,217,255)],"back":[(0,0,255,255)]*4}.items():
 image=QImage(2,2,QImage.Format_RGBA8888)
 for i,color in enumerate(colors):image.setPixelColor(i%2,i//2,QColor(*color))
 assert image.save(str(root/(name+".png")))
from core.image_plane import ImagePlane
if os.environ.get("INGETRAZO_IMAGE_UV_MIRROR_ABLATION")=="1":
 import inspect,textwrap,views.viewport as viewport_module
 namespace=dict(viewport_module.__dict__)
 texture_source=textwrap.dedent(inspect.getsource(Viewport._get_texture));assert "QOpenGLTexture(img.mirrored())" in texture_source
 exec(texture_source.replace("QOpenGLTexture(img.mirrored())","QOpenGLTexture(img)"),namespace)
 image_source=textwrap.dedent(inspect.getsource(Viewport._draw_image_planes));assert "v.x(), v.y(), v.z(), u, w" in image_source
 exec(image_source.replace("v.x(), v.y(), v.z(), u, w","v.x(), v.y(), v.z(), u, 1.0-w"),namespace)
 Viewport._get_texture=namespace["_get_texture"];Viewport._draw_image_planes=namespace["_draw_image_planes"]
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
if os.environ.get("INGETRAZO_IMAGE_GEOMETRY_CORPUS")=="1":
 recipes += [(name,texture,1.,False,"textures",True,None)for name,texture in [
 ("image-oblique","quadrants"),("image-perspective","quadrants"),("image-uv-asymmetric","quadrants"),("image-uv-asymmetric-perspective","quadrants"),("image-uv-negative-v","quadrants"),("image-offscreen-clip","quadrants"),("image-near-clipped","quadrants"),("image-tinted-alpha","tinted")]]
for name,texture,opacity,reverse,mode,show,layer in recipes:
 vp=Viewport();vp.resize(800,600);vp.scene=Scene(mesh=Mesh());
 if os.environ.get("INGETRAZO_IMAGE_OUTLINE_ABLATION")=="1":vp._draw_image_outlines=lambda painter:None
 if name.endswith("behind") or name=="image-coplanar-trace":
  z=0. if name=="image-coplanar-trace" else -.1
  face=vp.scene.mesh.add_face([QVector3D(-1,-1,z),QVector3D(1,-1,z),QVector3D(1,1,z),QVector3D(-1,1,z)]);face.attrs["color"]=(0.,0.,1.)
 im=ImagePlane(str(root/(texture+".png")),QVector3D(-1,-1,0) if not reverse else QVector3D(1,-1,0),QVector3D(2,0,0) if not reverse else QVector3D(-2,0,0),QVector3D(0,2,0));im.opacity=opacity;im.layer=layer;vp.scene.image_planes=[im]
 if name.startswith("image-uv-asymmetric"):im.origin=QVector3D(-1.1,-.6,.25);im.u=QVector3D(2.3,.2,.3);im.v=QVector3D(-.1,1.3,.4)
 if name=="image-uv-negative-v":im.origin=QVector3D(-1,1,0);im.u=QVector3D(2,0,0);im.v=QVector3D(0,-2,0)
 if name=="image-offscreen-clip":im.origin=QVector3D(.8,-1,0);im.u=QVector3D(4,0,0)
 if name=="image-near-clipped":im.origin=QVector3D(-1,-1,3.9);im.u=QVector3D(2,0,0);im.v=QVector3D(0,2,.5)
 if layer:
  from core.layers import Layer
  vp.scene.layers.append(Layer(layer,visible=False))
 if name=="image-overlap":
  front=ImagePlane(str(root/"alpha.png"),QVector3D(-1,-1,.1),QVector3D(2,0,0),QVector3D(0,2,0));front.opacity=.5;vp.scene.image_planes.append(front)
 vp.style_override=Style(face_mode=mode,sky=False,edges=False,profiles=False,background=(1.,1.,1.));camera_view="iso" if name in ("image-oblique","image-perspective","image-uv-asymmetric-perspective")else "top";vp.camera.set_view(camera_view);vp.camera.perspective=name in ("image-perspective","image-uv-asymmetric-perspective","image-near-clipped");vp.camera.target=QVector3D();vp.camera.distance=4.;vp.setWindowTitle("IngeTrazo owned image-plane reference")
 if os.environ.get("INGETRAZO_IMAGE_PROJECTION_Y_ABLATION"):
  original_projection=vp.camera.projection_matrix
  coefficient=float(os.environ["INGETRAZO_IMAGE_PROJECTION_Y_ABLATION"])
  def diagnostic_projection(original_projection=original_projection,coefficient=coefficient):
   m=original_projection();r=m.row(1);m.setRow(1,QVector4D(r.x(),coefficient,r.z(),r.w()));return m
  vp.camera.projection_matrix=diagnostic_projection
 vp.show()
 for _ in range(45):app.processEvents();time.sleep(.02)
 outline_segments=[]
 for plane_index,plane in enumerate(vp.scene.image_planes):
  if not vp.scene.entity_visible(plane):continue
  for a,b in plane.border_edges():
   clipped=vp._clip_segment_front(a,b)
   if clipped is None:continue
   pa,pb=map(vp._world_to_pixel,clipped)
   if pa is not None and pb is not None:
    outline_segments.append({"plane":plane_index,"a":[a.x(),a.y(),a.z()],"b":[b.x(),b.y(),b.z()],"clipped_a":[clipped[0].x(),clipped[0].y(),clipped[0].z()],"clipped_b":[clipped[1].x(),clipped[1].y(),clipped[1].z()],"screen_a":pa,"screen_b":pb})
 image=vp.grabFramebuffer()
 vp.makeCurrent();gl=ctypes.CDLL("libGL.so.1");scene_fbo=vp._scene_fbo;assert scene_fbo is not None;scene_fbo.bind()
 sample_count=ctypes.c_int();sample_buffers=ctypes.c_int();gl.glGetIntegerv(0x80A9,ctypes.byref(sample_count));gl.glGetIntegerv(0x80A8,ctypes.byref(sample_buffers));sample_positions=[]
 for sample_index in range(sample_count.value):
  pos=(ctypes.c_float*2)();gl.glGetMultisamplefv(0x8E50,sample_index,pos);sample_positions.append(list(pos))
 scene_fbo.release();vp.doneCurrent()
 if os.environ.get("INGETRAZO_REQUIRE_NVIDIA")=="1":assert "NVIDIA GeForce RTX 5090" in vp.gl_info["renderer"],vp.gl_info
 assert not image.isNull();image.save(str(root/(name+".png")));save_scene(vp.scene,root/(name+".igz"))
 points=[(340,240),(460,240),(340,360),(460,360),(400,300)];samples=[]
 for x,y in points:c=image.pixelColor(x,y);samples.append([c.red(),c.green(),c.blue(),c.alpha()])
 rows.append({"case":name,"background":[1,1,1],"camera_view":camera_view,"camera_perspective":vp.camera.perspective,"points":points,"samples":samples,"gl_info":vp.gl_info,"actual_gl_samples":sample_count.value,"actual_gl_sample_buffers":sample_buffers.value,"qt_surface_samples":vp.format().samples(),"scene_fbo_samples":scene_fbo.format().samples(),"gl_sample_positions":sample_positions,"uv_mirror_ablation":os.environ.get("INGETRAZO_IMAGE_UV_MIRROR_ABLATION")=="1","projection_y_ablation":os.environ.get("INGETRAZO_IMAGE_PROJECTION_Y_ABLATION"),"image_outline_ablation":os.environ.get("INGETRAZO_IMAGE_OUTLINE_ABLATION")=="1","outline_segments":outline_segments,"projection_matrix":list(vp.camera.projection_matrix().data()),"view_matrix":list(vp.camera.view_matrix().data()),"mvp":list((vp.camera.projection_matrix()*vp.camera.view_matrix()).data()),"style_mode":{"wireframe":"wireframe","hidden":"hidden_line","xray":"xray"}.get(mode,"textures"),"show_textures":show,"native_input_sha256":hashlib.sha256((root/(name+".igz")).read_bytes()).hexdigest()})
 vp.close();vp.deleteLater();app.processEvents()
report={"cases":rows,"scope":f"Actual original {len(rows)} image-plane controls; no full GPU image parity claim"};(root/"original-alpha-results.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
