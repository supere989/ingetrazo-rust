#!/usr/bin/env python3
"""Actual frozen Qt viewport loose-surface interior colors; not whole-widget parity."""
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
from views.viewport import Viewport
import numpy as np
scene=Scene();scene.layers.append(Layer('loose'))
face=scene.mesh.add_face([QVector3D(-.6,-.6,0),QVector3D(.6,-.6,0),QVector3D(0,.6,0)])
face.attrs.update({'color':(1.,0.,0.),'layer':'loose'})
vp=Viewport();vp.resize(800,600);vp.scene=scene;vp.style_override=Style(sky=False,edges=False,profiles=False,background=(0.,0.,1.));vp.camera.target=QVector3D();vp.camera.distance=8.;vp.camera.yaw=-math.pi/2;vp.camera.pitch=math.pi/2;vp.camera.perspective=False;vp.show()
rows=[]
def capture(name,x):
 scene.version+=1;scene.mesh._chunk_dirty=True;scene.mesh._mut_serial+=1;vp.update()
 # Bounded settle; exactly one capture per phase, no image-based retry.
 end=time.monotonic()+.15
 while time.monotonic()<end:app.processEvents();time.sleep(.005)
 image=vp.grabFramebuffer();assert not image.isNull() and (image.width(),image.height())==(800,600)
 assert image.save(str(out/(name+'.png')))
 px,py,visible=vp._project_px(np.array([[x,-.2,0.]],dtype=float));assert bool(visible[0]);cx,cy=round(float(px[0])),round(float(py[0]))
 pixels=[[image.pixelColor(xx,yy).red(),image.pixelColor(xx,yy).green(),image.pixelColor(xx,yy).blue(),image.pixelColor(xx,yy).alpha()]for yy in range(cy-1,cy+2)for xx in range(cx-1,cx+2)]
 assert all(p==pixels[0] for p in pixels),'sample crossed a primitive edge'
 rows.append({'name':name,'rgba':pixels[0],'pixel':[cx,cy],'png_sha256':hashlib.sha256((out/(name+'.png')).read_bytes()).hexdigest()})
capture('loose-initial',0);capture('loose-warm',0)
for vertex in scene.mesh.vertices:vertex.position+=QVector3D(.8,0,0)
capture('loose-geometry',.8)
face.attrs['color']=(1.,1.,0.);capture('loose-paint',.8)
scene.layers[-1].visible=False;capture('loose-layer-hidden',.8)
scene.layers[-1].visible=True;capture('loose-layer-restored',.8)
scene.mesh=Mesh();capture('loose-topology-removed',.8)
vp.close();app.processEvents()
report={'source_pin':pin,'passed':True,'cases':rows,'scope':'Actual frozen Qt viewport OpenGL/MSAA interior RGBA; source constructor/mesh edits and explicit version notification, no delivered user gesture or whole-item alignment claim'}
(out/'source.json').write_text(json.dumps(report,indent=2)+'\n')
print([(r['name'],r['rgba'])for r in rows])
