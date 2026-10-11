#!/usr/bin/env python3
"""Literal pinned-source exporter recipes; generates authoritative input IGZs."""
import sys
from pathlib import Path
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtGui import QVector3D as V,QMatrix4x4,QImage,QColor
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.materials import Material
from core.layers import Layer
from formats.igz import save_scene,load_into
from formats.gltf import save_glb
from georef.datum import SceneDatum
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
def face(mesh,x=0,attrs=None,holes=None):
    f=mesh.add_face([V(x,0,0),V(x+3,0,0),V(x+3,2,0),V(x,2,0)],holes)
    f.attrs.update(attrs or {});return f
img=QImage(5,3,QImage.Format_RGBA8888)
for y in range(3):
    for x in range(5):img.setPixelColor(x,y,QColor(20+x*30,70+y*40,170,0 if x==2 else 255))
img.save(str(out/'leaves.png'),'PNG')
img.fill(QColor(35,120,205));img.save(str(out/'opaque.jpg'),'JPEG')
cases=[]
s=Scene();face(s.mesh);cases.append(('default',s))
s=Scene()
for i,(name,finish,color,op) in enumerate([('1 copper','metal',(.2,.3,.4),None),('glass wall','glass',(.7,.8,.9),.3335),('pool water','water',(.1,.4,.6),None),('wood floor','auto',(.3,.2,.1),None),('same name','satin',(.8,.2,.3),.999),('same name','satin',(.2,.8,.3),None)]):
    s.materials[name]=Material(name,color=color,finish=finish)
    attrs={'color':color,'mat':name,'back_color':(1.,0.,1.)}
    if op is not None:attrs['opacity']=op
    face(s.mesh,i*4,attrs)
s.georef=SceneDatum(-16.4,-71.5,2340,12)
cases.append(('pbr-colors',s))
s=Scene()
for i,tex in enumerate([{'path':str(out/'leaves.png'),'sw':2,'sh':3,'rot':30},{'path':str(out/'opaque.jpg'),'uvw':[.5,0,0,.25,0,.5,0,.125]},{'path':str(out/'leaves.png'),'sw':1,'sh':1},{'path':str(out/'absent.png'),'sw':1,'sh':1}]):
    attrs={'texture':tex,'mat':'leaves' if i!=1 else 'ceramic panel'}
    if i==2:attrs['opacity']=.42
    face(s.mesh,4*i,attrs)
cases.append(('textures-alpha',s))
s=Scene();m=Mesh();face(m,attrs={'color':(.25,.5,.75),'texture':{'path':str(out/'opaque.jpg'),'uvw':[.5,0,0,0,0,.5,0,0]}})
g=Group(m,'leaf');g.xform=QMatrix4x4();g.xform.translate(2,3,4);g.xform.scale(-2,3,.5)
p=Group(Mesh(),'parent');p.xform=QMatrix4x4();p.xform.translate(8,-2,1);p.xform.rotate(90,0,0,1);p.children=[g]
s.groups=[p];cases.append(('nested-reflected',s))
s=Scene();face(s.mesh,0,{'hidden':True});face(s.mesh,4,{'layer':'off'});face(s.mesh,8,{'color':(.1,.2,.3)});s.layers.append(Layer('off',visible=False))
g=Group(Mesh(),'hidden root');face(g.mesh,12);g.hidden=True;s.groups.append(g)
g=Group(Mesh(),'billboard');face(g.mesh,16);g.billboard=True;s.groups.append(g)
p=Group(Mesh(),'parent');p.xform=QMatrix4x4();g=Group(Mesh(),'hidden child');face(g.mesh,20,{'hidden':True,'color':(.7,.4,.2)});g.hidden=True;p.children=[g];s.groups.append(p);cases.append(('visibility',s))
s=Scene();f=s.mesh.add_face([V(0,0,0),V(5,0,0),V(5,5,0),V(0,5,0)],[[V(1,1,0),V(1,3,0),V(3,3,0),V(3,1,0)]]);f.attrs={'color':(.2,.4,.6)};cases.append(('hole',s))
s=Scene();m=Mesh();f=m.add_face([V(0,0,0),V(2,0,1),V(2,3,1),V(0,3,0)]);f.attrs={'texture':{'path':str(out/'opaque.jpg'),'sw':1.25,'sh':2.5,'rot':17}};s.groups=[Group(m,'slanted')];cases.append(('slanted',s))
s=Scene();m=Mesh();f=m.add_face([V(.125,.25,.5),V(2.125,.25,1.5),V(2.125,3.25,1.5),V(.125,3.25,.5)]);f.attrs={'texture':{'path':str(out/'opaque.jpg'),'uvw':[.5,.125,.25,.15,-.125,.25,.5,.35]},'mat':'wood opaque'}
g=Group(m,'rotated leaf');g.xform=QMatrix4x4();g.xform.translate(.3,-.7,2.125);g.xform.rotate(23,0,0,1);g.xform.scale(-1.125,.75,1.5)
p=Group(Mesh(),'rotated parent');p.xform=QMatrix4x4();p.xform.translate(3.125,1.25,-.5);p.xform.rotate(17,0,1,0);p.children=[g];s.groups=[p];cases.append(('affine-nested',s))
cases.append(('empty',Scene()))
for name,s in cases:
    save_scene(s,out/(name+'.igz'));save_glb(s,out/(name+'.authored.glb'))
    loaded=Scene();load_into(loaded,out/(name+'.igz'));save_glb(loaded,out/(name+'.glb'))
print('\n'.join(name for name,_ in cases))
