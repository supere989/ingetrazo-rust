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
rows=[]
for name,texture,opacity,repeat,reverse,back in [("texture-quadrants","quadrants",1.,1.,False,None),("texture-repeat","quadrants",1.,2.,False,None),("texture-alpha","alpha",.5,1.,False,None),("texture-cutout","cutout",1.,1.,False,None),("texture-back","quadrants",1.,1.,True,"back")]:
 mesh=Mesh();ring=[QVector3D(-1,-1,0),QVector3D(1,-1,0),QVector3D(1,1,0),QVector3D(-1,1,0)]
 if reverse:ring.reverse()
 face=mesh.add_face(ring);face.attrs.update(texture={"path":str(root/(texture+".png")),"uvw":[repeat/2,0,0,repeat/2,0,repeat/2,0,repeat/2]},opacity=opacity)
 if back:face.attrs["back"]={"texture":{"path":str(root/(back+".png")),"uvw":[.5,0,0,.5,0,.5,0,.5]},"opacity":1.}
 vp=Viewport();vp.resize(800,600);vp.scene=Scene(mesh=mesh);vp.style_override=Style(face_mode="textures",sky=False,edges=False,profiles=False,background=(1.,1.,1.));vp.camera.set_view("top");vp.camera.perspective=False;vp.camera.target=QVector3D();vp.camera.distance=4.;vp.setWindowTitle("IngeTrazo owned NVIDIA texture reference");vp.show()
 for _ in range(45):app.processEvents();time.sleep(.02)
 image=vp.grabFramebuffer();
 if os.environ.get("INGETRAZO_REQUIRE_NVIDIA")=="1":assert "NVIDIA GeForce RTX 5090" in vp.gl_info["renderer"],vp.gl_info
 assert not image.isNull();image.save(str(root/(name+".png")));save_scene(vp.scene,root/(name+".igz"))
 points=[(340,240),(460,240),(340,360),(460,360),(400,300)];samples=[]
 for x,y in points:c=image.pixelColor(x,y);samples.append([c.red(),c.green(),c.blue(),c.alpha()])
 rows.append({"case":name,"background":[1,1,1],"points":points,"samples":samples,"gl_info":vp.gl_info,"native_input_sha256":hashlib.sha256((root/(name+".igz")).read_bytes()).hexdigest()})
 vp.close();vp.deleteLater();app.processEvents()
report={"cases":rows,"scope":"Actual original QOpenGLWidget five initial literal texture controls; source repeat/UV/alpha/back/cutout samples, not full texture parity"};(root/"original-alpha-results.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
