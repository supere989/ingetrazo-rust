#!/usr/bin/env python3
"""Pinned actual world texture filter/minification and multi-asset upload inputs."""
import sys,time,json,hashlib
from pathlib import Path
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtGui import QSurfaceFormat,QImage,QColor,QVector3D
from PySide6.QtWidgets import QApplication
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setDepthBufferSize(24);fmt.setStencilBufferSize(8);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from views.viewport import Viewport
from core.mesh import Mesh
from core.scene import Scene
from core.style import Style
from formats.igz import save_scene
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
assets=[]
for i,(w,h) in enumerate([(32,32),(64,64),(256,256),(128,64),(8,8)]):
 image=QImage(w,h,QImage.Format_RGBA8888)
 for y in range(h):
  for x in range(w):
   if i==4:color=(255,0,0,255) if x<4 else (0,0,0,0)
   else:color=((31+37*i,83+19*i,137-21*i,255) if (x+y)%2 else (207-11*i,191-23*i,49+31*i,255))
   image.setPixelColor(x,y,QColor(*color))
 path=out/f'asset-{i}.png';assert image.save(str(path));assets.append(path)
mesh=Mesh()
for i,path in enumerate(assets):
 x=-1.5+i*.6
 face=mesh.add_face([QVector3D(x,-.5,0),QVector3D(x+.5,-.5,0),QVector3D(x+.5,.5,0),QVector3D(x,.5,0)])
 face.attrs['texture']={'path':str(path),'uvw':[32,0,0,32,0,32,0,32]}
vp=Viewport();vp.resize(800,600);vp.setFixedSize(800,600);vp.scene=Scene(mesh=mesh);vp.style_override=Style(face_mode='textures',sky=False,edges=False,profiles=False,background=(1.,1.,1.));vp.camera.set_view('top');vp.camera.perspective=False;vp.camera.target=QVector3D();vp.camera.distance=4.;vp.show()
rows=[]
for distance in [4.,12.]:
 vp.camera.distance=distance;vp.update()
 for _ in range(30):app.processEvents();time.sleep(.02)
 image=vp.grabFramebuffer();assert 'NVIDIA GeForce RTX 5090' in vp.gl_info['renderer'],vp.gl_info;assert image.size().width()==800 and image.size().height()==600
 name='near' if distance==4 else 'far';image.save(str(out/f'{name}.png'))
 points=[]
 for i in range(5):
  for dx in [-.08,0,.08]:
   pos=vp._world_to_pixel(QVector3D(-1.25+i*.6+dx,0,0));points.append([round(pos[0]),round(pos[1])])
 samples=[]
 for x,y in points:
  c=image.pixelColor(x,y);samples.append([c.red(),c.green(),c.blue(),c.alpha()])
 filters=[]
 for path in assets:
  tex=vp._get_texture(str(path));filters.append({'min':int(tex.minificationFilter().value),'mag':int(tex.magnificationFilter().value),'levels':tex.mipLevels(),'cutout':bool(getattr(tex,'_cutout',False))})
 rows.append({'case':name,'distance':distance,'points':points,'samples':samples,'filters':filters,'gl_info':vp.gl_info})
save_scene(vp.scene,out/'batch.igz');vp.close();app.processEvents()
report={'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','cases':rows,'igz_sha256':hashlib.sha256((out/'batch.igz').read_bytes()).hexdigest(),'assets':[{'name':p.name,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in assets]}
(out/'original.json').write_text(json.dumps(report,indent=2)+'\n')
