#!/usr/bin/env python3
"""Missing camera-facing export contracts; unchanged pinned original handlers."""
import sys,json
from pathlib import Path
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtGui import QVector3D as V,QMatrix4x4,QImage,QColor
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.camera import OrbitCamera
from core.layers import Layer
from core.materials import Material
from formats.igz import save_scene,load_into
from formats.gltf import save_glb
from core.render_blender import camera_dict
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
img=QImage(5,3,QImage.Format_RGBA8888)
for y in range(3):
 for x in range(5):img.setPixelColor(x,y,QColor(25+x*35,60+y*45,180,0 if x==2 else 255))
img.save(str(out/'figure.png'))
def mesh(attrs=None):
 m=Mesh();f=m.add_face([V(-1,0,0),V(1,0,0),V(1,0,3),V(-1,0,3)]);f.attrs.update(attrs or {});return m
def figure(kind='mesh',texture=False,name='figure'):
 a={'color':(.2,.4,.7),'mat':'shared'}
 if texture:a['texture']={'path':str(out/'figure.png'),'sw':2,'sh':3,'uvw':[.5,0,0,.5,0,0,1/3,0]}
 g=Group(mesh(a),name);g.billboard=kind;return g
cases=[]
s=Scene();s.groups=[figure()];cases.append(('mesh-perspective',s,True))
s=Scene();s.groups=[figure(True,True)];cases.append(('bitmap-perspective',s,True))
s=Scene();p=Group(mesh({'color':(.7,.2,.1)}),'parent');p.xform=QMatrix4x4();p.xform.translate(2,-1,.5);p.xform.rotate(90,0,0,1)
f=figure('mesh');f.hidden=True;f.layer='off';f.xform=QMatrix4x4();f.xform.translate(.5,2,1);p.children=[f];s.groups=[p];s.layers.append(Layer('off',visible=False));cases.append(('nested-hidden-child',s,False))
s=Scene();g=figure();g.hidden=True;s.groups=[g];m=figure();m.layer='off';s.groups.append(m);s.layers.append(Layer('off',visible=False));cases.append(('hidden-top',s,True))
s=Scene();s.groups=[figure(True,True),figure('mesh')];s.groups[1].xform=QMatrix4x4();s.groups[1].xform.translate(3,0,0);cases.append(('figure-only',s,False))
s=Scene();s.mesh=mesh({'color':(.2,.4,.7),'mat':'ordinary-first'});s.materials['ordinary-first']=Material('ordinary-first',color=(.2,.4,.7),finish='metal');s.materials['shared']=Material('shared',color=(.2,.4,.7),finish='glass');f=figure();f.xform=QMatrix4x4();f.xform.translate(3,0,0);f.children=[Group(mesh({'color':(.8,.3,.1)}),'ordinary child')];s.groups=[f,figure(True,True)];cases.append(('mixed-first-material',s,True))
s=Scene();s.groups=[figure(True)];cases.append(('bitmap-no-texture',s,False))
s=Scene();g=figure();g.mesh.vertex(V(20,30,-7));g.xform=QMatrix4x4();s.groups=[g];cases.append(('identity-orphan',s,True))
records=[]
for name,s,perspective in cases:
 cam=OrbitCamera();cam.target=V(.25,-.5,1);cam.distance=7;cam.yaw=.4;cam.pitch=.2;cam.perspective=perspective
 data=dict(target=[cam.target.x(),cam.target.y(),cam.target.z()],distance=cam.distance,yaw=cam.yaw,pitch=cam.pitch,perspective=cam.perspective)
 (out/(name+'.camera.json')).write_text(json.dumps(data,indent=2)+'\n')
 save_scene(s,out/(name+'.igz'));loaded=Scene();load_into(loaded,out/(name+'.igz'))
 eye=cam.eye();toward=lambda feet:eye-feet if cam.perspective else eye-cam.target
 save_glb(loaded,out/(name+'.glb'),face_me=toward)
 save_glb(loaded,out/(name+'.ordinary.glb'))
 records.append(dict(name=name,camera=data,resolved=camera_dict(cam)))
(out/'cases.json').write_text(json.dumps(records,indent=2)+'\n')
print(json.dumps(dict(cases=len(records),names=[r['name'] for r in records])))
