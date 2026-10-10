#!/usr/bin/env python3
"""Read authored schema through frozen original, then render only own NVIDIA window."""
import sys,json,copy,time,hashlib
from pathlib import Path
from acceptance_context import context
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtGui import QSurfaceFormat,QVector3D,QMatrix4x4
from PySide6.QtWidgets import QApplication
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setDepthBufferSize(24);fmt.setStencilBufferSize(8);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from core.scene import Scene
from core.mesh import Mesh
from core.style import Style
from formats.igz import load_into,save_scene
from views.viewport import Viewport
root=Path(sys.argv[1]);root.mkdir(parents=True,exist_ok=True)
face={'vertices':[[-1.,-1.,0.],[1.,-1.,0.],[1.,1.,0.],[-1.,1.,0.]],'color':[1.,0.,0.],'opacity':1.,'back':{'color':[0.,0.,1.],'opacity':.3}}
opaque={'edges':[],'faces':[{'vertices':[[9.,-1.,0.],[11.,-1.,0.],[11.,1.,0.],[9.,1.,0.]],'color':[0.,1.,0.],'opacity':1.}],'name':'Opaque activator','uid':'audit-opaque','component':False,'ext':{'vendor_token':71}}
identity=[1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.]
base={'igz_format':1,'app_version':'0.5.7','scene':{'edges':[],'faces':[face],'groups':[opaque]}}
cases={}
def add(name,doc):cases[name]=copy.deepcopy(doc)
add('authored-legacy-absent',base)
d=copy.deepcopy(base);d['scene']['groups'][0]['proto']=None;add('authored-legacy-null',d)
d['scene']['groups'][0]['xform']=None;add('authored-legacy-null-nullxform',d)
d['scene']['groups'][0]['component']=True;add('authored-legacy-null-component-true',d)
for component in [True,False]:
 d=copy.deepcopy(base);d['scene']['protos']=[copy.deepcopy(opaque)];d['scene']['groups']=[{'proto':0,'xform':identity,'name':'Numeric zero','uid':'audit-numeric','component':component,'ext':{'vendor_token':72}}];add('authored-proto-zero-'+str(component).lower(),d)
d=copy.deepcopy(base);d['scene']['protos']=[{'edges':[],'faces':[]},copy.deepcopy(opaque)]
xf=identity.copy();xf[0]=2.;xf[5]=.5;xf[12]=3.
child={'proto':1,'xform':xf,'name':'Numeric one child','uid':'audit-child','component':True,'ext':{'vendor_token':73}}
d['scene']['groups']=[{'name':'Adopt parent','uid':'audit-parent','component':False,'children':[child]}];add('authored-nested-numeric-adopt',d)
d=copy.deepcopy(base);leaf=d['scene']['groups'][0];leaf['proto']=None;d['scene']['groups']=[{'name':'Classic adopt parent','uid':'audit-parent','component':False,'children':[leaf]}];add('authored-nested-null-adopt',d)
def observations(scene):
 rows=[]
 def visit(group,parent,path):
  xf=group.xform;world=parent if xf is None else parent*xf
  points=[]
  for f in group.mesh.faces:
   points.extend([[float(p.x()),float(p.y()),float(p.z())]for v in f.vertices for p in [world.map(v)]])
  rows.append({'path':path,'uid':group.uid,'component':bool(group.component),'is_component':bool(group.is_component()),'xform':None if xf is None else [float(x)for x in xf.data()],'world_points':sorted(points),'ext':group.ext})
  for i,c in enumerate(group.children):visit(c,world,path+[i])
 for i,g in enumerate(scene.groups):visit(g,QMatrix4x4(),[i])
 return rows
rows=[]
for name,doc in cases.items():
 path=root/(name+'.igz');path.write_text(json.dumps(doc,separators=(',',':')))
 scene=Scene();load_into(scene,path)
 obs=observations(scene);save_scene(scene,root/(name+'.original-roundtrip.igz'))
 reopened=Scene();load_into(reopened,root/(name+'.original-roundtrip.igz'));roundtrip=observations(reopened)
 assert [{k:v for k,v in r.items()if k!='component'}for r in roundtrip]==[{k:v for k,v in r.items()if k!='component'}for r in obs],(name,obs,roundtrip)
 vp=Viewport();vp.setWindowTitle('IngeTrazo original authored schema probe');vp.resize(800,600);vp.scene=scene
 vp.style_override=Style(face_mode='shaded',sky=False,edges=False,profiles=False,background=(1.,1.,1.));vp.camera.set_view('top');vp.camera.perspective=False;vp.camera.target=QVector3D(0,0,0);vp.camera.distance=4.;vp.show()
 for _ in range(45):app.processEvents();time.sleep(.02)
 im=vp.grabFramebuffer();assert not im.isNull();assert 'NVIDIA'in vp.gl_info['renderer'] and not vp.gl_info['software'];im.save(str(root/(name+'.png')));c=im.pixelColor(400,300)
 rows.append({'case':name,'center_rgba':[c.red(),c.green(),c.blue(),c.alpha()],'gl_info':vp.gl_info,'background':[1.,1.,1.],'native_input_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'groups':obs,'original_roundtrip_groups':roundtrip,'ordinary_faces_count':vp._faces_count})
 vp.close();vp.deleteLater();app.processEvents()
assert len(rows)==8
(root/'original-alpha-results.json').write_text(json.dumps({'provenance':context(),'cases':rows,'scope':'Authored schema loaded by actual original; unmodified raw document supplied to native; original roundtrip semantics checked.'},indent=2)+'\n')
print(json.dumps(rows,indent=2))
