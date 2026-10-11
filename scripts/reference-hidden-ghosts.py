#!/usr/bin/env python3
"""Pinned actual Qt ghost pass, owned input-transparent window; new targeted corpus."""
import ctypes,hashlib,json,os,sys,time
from pathlib import Path
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtGui import QSurfaceFormat,QVector3D as V,QMatrix4x4,QImage,QColor
from PySide6.QtCore import Qt
from PySide6.QtWidgets import QApplication
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setSamples(4);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from views.viewport import Viewport
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.style import Style
from formats.igz import save_scene
root=Path(sys.argv[1]).resolve();root.mkdir(parents=True,exist_ok=True)
gl=ctypes.CDLL('libGL.so.1')
class Observer(Viewport):
 def _draw_hidden_ghosts(self,mvp):
  n=ctypes.c_int();gl.glGetIntegerv(0x80a9,ctypes.byref(n));self.ghost_samples=n.value
  return super()._draw_hidden_ghosts(mvp)
def square(mesh,z=0):return mesh.add_face([V(-1,-1,z),V(1,-1,z),V(1,1,z),V(-1,1,z)])
rows=[]
for kind in ['face','edge','object','textured-object','nested-object','occluded-face']:
 for shown in [False,True]:
  scene=Scene();mesh=Mesh() if 'object' in kind else scene.mesh
  if kind=='edge':mesh.add_edge(V(-1,-.2,0),V(1,.2,0)).hidden=True
  else:
   face=square(mesh);face.attrs['color']=(.8,.3,.1)
   if 'object' not in kind:face.attrs['hidden']=True
  if kind=='occluded-face':square(scene.mesh,.25).attrs['color']=(.2,.5,.7)
  if 'object' in kind:
   group=Group(mesh,'hidden');group.hidden=True
   if kind=='textured-object':face.attrs['texture']={'path':'missing-ghost-image.png','sw':2.,'sh':2.}
   if kind=='nested-object':
    child=group;child.hidden=False;group=Group(name='parent');group.hidden=True;group.children=[child];group.xform=QMatrix4x4();group.xform.translate(.25,0,0)
   scene.groups=[group]
  scene.show_hidden_objects=shown;scene.show_hidden_geometry=shown
  name=kind+('-on' if shown else '-off')
  vp=Observer();vp.setWindowFlags(vp.windowFlags()|Qt.WindowTransparentForInput|Qt.WindowDoesNotAcceptFocus);vp.setFixedSize(800,600)
  vp.scene=scene;vp.style_override=Style(face_mode='textures',sky=False,edges=False,profiles=False,background=(1.,1.,1.));scene.display_style=vp.style_override
  vp.camera.set_view('top');vp.camera.perspective=False;vp.camera.target=V();vp.camera.distance=4.;vp.show()
  end=time.monotonic()+.4
  while time.monotonic()<end:app.processEvents();time.sleep(.005)
  image=vp.grabFramebuffer();assert image.size().width()==800 and image.size().height()==600
  if os.environ.get('INGETRAZO_REQUIRE_NVIDIA')=='1':assert vp.gl_info['vendor']=='NVIDIA Corporation' and 'NVIDIA GeForce RTX 5090' in vp.gl_info['renderer']
  image.save(str(root/(name+'.png')));save_scene(scene,root/(name+'.igz'))
  points=[(340,240),(460,240),(340,360),(460,360),(400,300)];samples=[[image.pixelColor(x,y).red(),image.pixelColor(x,y).green(),image.pixelColor(x,y).blue(),image.pixelColor(x,y).alpha()]for x,y in points]
  rows.append({'case':name,'background':[1,1,1],'show_edges':False,'points':points,'samples':samples,'ghost_samples':vp.ghost_samples,'gl_info':vp.gl_info,'native_input_sha256':hashlib.sha256((root/(name+'.igz')).read_bytes()).hexdigest()})
  vp.close();vp.deleteLater();app.processEvents()
(root/'original-alpha-results.json').write_text(json.dumps({'cases':rows,'scope':'actual original ghost pass on/off; only owned window; hidden picking separately required'},indent=2)+'\n')
