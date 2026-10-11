#!/usr/bin/env python3
"""Actual frozen Qt viewport classic baked own-mesh interior colors; not whole-widget parity."""
import sys,json,subprocess,hashlib,time,math
from pathlib import Path
root=Path(__file__).resolve().parents[1];source=root/'ingetrazo';out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
pin='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()==pin
sys.path.insert(0,str(source))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QSurfaceFormat,QVector3D
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setDepthBufferSize(24);fmt.setSamples(4);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from core.scene import Scene
from core.style import Style
from core.layers import Layer
from core.mesh import Mesh
from core.group import Group
from views.viewport import Viewport
import numpy as np
scene=Scene()
# Frozen source image bytes, authored UV covectors and material recipes.
from PySide6.QtGui import QImage,QColor,QMatrix4x4
asset=QImage(2,2,QImage.Format_RGBA8888)
for y,row in enumerate([[(255,0,0),(0,255,0)],[(0,0,255),(255,255,0)]]):
 for x,color in enumerate(row):asset.setPixelColor(x,y,QColor(*color))
assert asset.save(str(out/'quadrants.png'))
path=str(out/'quadrants.png')
texture=lambda u,v:{'path':path,'uvw':[0.,0.,0.,u,0.,0.,0.,v]}
recipes=[
 ('shaded-transparent-texture',{'texture':texture(.25,.75),'opacity':.5},False,'shaded'),
 ('shaded-back-texture',{'texture':texture(.25,.25),'back':{'texture':texture(.25,.75)}},True,'shaded')]
vp=Viewport();vp.resize(800,600);vp.scene=scene;vp.camera.target=QVector3D();vp.camera.distance=8.;vp.camera.yaw=-math.pi/2;vp.camera.pitch=math.pi/2;vp.camera.perspective=False;vp.show()
rows=[]
for name,attrs,reverse,mode in recipes:
 mesh=Mesh();points=[QVector3D(x,y,0.)for x,y in [(-.6,-.6),(.6,-.6),(0.,.6)]]
 if reverse:points.reverse()
 face=mesh.add_face(points);scene.mesh=mesh;scene.groups=[]
 if name.startswith('group-'):
  group=Group(mesh,'material');group.xform=QMatrix4x4();group.material=attrs.copy()
  if name=='group-reflected-texture':group.xform.scale(-1.,1.,1.)
  scene.mesh=Mesh();scene.groups=[group]
 else:face.attrs.update(attrs)
 vp.style_override=Style(sky=False,edges=False,profiles=False,background=(0.,0.,1.),face_mode=mode)
 scene.version+=1;vp.update();end=time.monotonic()+.15
 while time.monotonic()<end:app.processEvents();time.sleep(.005)
 image=vp.grabFramebuffer();assert not image.isNull();assert image.save(str(out/(name+'.png')))
 px,py,visible=vp._project_px(np.array([[0.,-.2,0.]],dtype=float));assert visible[0];cx,cy=round(float(px[0])),round(float(py[0]))
 pixels=[[image.pixelColor(xx,yy).red(),image.pixelColor(xx,yy).green(),image.pixelColor(xx,yy).blue(),image.pixelColor(xx,yy).alpha()]for yy in range(cy-1,cy+2)for xx in range(cx-1,cx+2)]
 assert all(p==pixels[0]for p in pixels),name
 rows.append({'name':name,'rgba':pixels[0],'pixel':[cx,cy],'png_sha256':hashlib.sha256((out/(name+'.png')).read_bytes()).hexdigest(),'reverse':reverse,'mode':mode,'attrs':attrs})
vp.close();app.processEvents()
report={'source_pin':pin,'passed':True,'asset_sha256':hashlib.sha256((out/'quadrants.png').read_bytes()).hexdigest(),'cases':rows,'scope':'Actual frozen Qt OpenGL viewport sampled interior RGBA, authored constant/repeat UV covectors, color/alpha/front/back/texture mean; no whole image or mip parity claim'}
(out/'source.json').write_text(json.dumps(report,indent=2)+'\n')
print([(r['name'],r['rgba'])for r in rows])
