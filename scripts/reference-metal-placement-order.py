#!/usr/bin/env python3
"""Focused actual frozen Qt instanced coplanar default-back order witness."""
import hashlib,json,math,pathlib,subprocess,sys,time
root=pathlib.Path(__file__).resolve().parents[1]
source=root/'ingetrazo';pin=subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()
assert pin=='6be29fe437a7cd992d4e079f71720821b25216b9'
sys.path.insert(0,str(source))
from PySide6.QtGui import QSurfaceFormat,QVector3D,QMatrix4x4
from PySide6.QtWidgets import QApplication
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setDepthBufferSize(24);fmt.setSamples(0);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.style import Style
import views.viewport as module
out=pathlib.Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
mesh=Mesh()
# Back-facing larger red face authored first, front-facing smaller green second.
back=mesh.add_face([QVector3D(-1,-1,0),QVector3D(0,1,0),QVector3D(1,-1,0)])
back.attrs['color']=[1.,0.,0.]
front=mesh.add_face([QVector3D(-.7,-.7,0),QVector3D(.7,-.7,0),QVector3D(0,.7,0)])
front.attrs['color']=[0.,1.,0.]
assert len(mesh.faces)==2
s=Scene();g=Group(mesh,'coplanar-order');g.xform=QMatrix4x4();s.groups=[g]
style=Style(face_mode='textures',edges=False,profiles=False,sky=False,background=(0.,0.,1.),back_color=(1.,1.,0.));s.display_style=style
vp=module.Viewport();vp.setMinimumSize(0,0);vp.resize(400,400);vp.scene=s;vp.style_override=style
vp.camera.target=QVector3D(0,0,0);vp.camera.distance=4;vp.camera.pitch=math.pi/2;vp.camera.yaw=0;vp.camera.perspective=False;vp.show()
rows=[]
for name,disable in [('source-late-back',False),('missing-late-back-negative',True)]:
 module._NO_BACK_TINT=disable;vp.update();deadline=time.monotonic()+1
 while time.monotonic()<deadline:app.processEvents();time.sleep(.01)
 assert vp.isValid() and vp._instanced_eligible(g)
 im=vp.grabFramebuffer();assert im.size().width()==400 and im.size().height()==400
 path=out/(name+'.png');assert im.save(str(path))
 pixels=[list(im.pixelColor(x,y).getRgb()) for y in range(195,205) for x in range(195,205)]
 expected=[255,255,0,255] if not disable else [0,255,0,255]
 # Front vertex shade is source-owned; record exact observed green magnitude.
 if disable:assert all(p[0]==0 and p[1]>100 and p[2]==0 and p[3]==255 for p in pixels),pixels
 else:assert all(p==expected for p in pixels),pixels
 rows.append({'name':name,'roi':[195,195,10,10],'pixels':pixels,'png_sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
assert rows[0]['pixels']!=rows[1]['pixels']
vp.close();vp.deleteLater();app.processEvents()
(out/'review.json').write_text(json.dumps({'source_pin':pin,'passed':True,'cases':rows,'scope':'actual original Qt instanced coplanar face ordering, interior only; native comparator still required','viewport_source_sha256':hashlib.sha256((source/'views/viewport.py').read_bytes()).hexdigest()},indent=2)+'\n')
