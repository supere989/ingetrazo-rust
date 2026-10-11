#!/usr/bin/env python3
"""Actual pinned Qt preview passes; owned input-transparent window only."""
import ctypes,hashlib,json,math,sys,time,copy
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from PySide6.QtGui import QSurfaceFormat,QVector3D as V,QImage,QColor
from PySide6.QtCore import Qt
from PySide6.QtWidgets import QApplication
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setSamples(4);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from views.viewport import Viewport
from core.scene import Scene
from core.style import Style
from formats.igz import save_scene
from tests.test_pushpull_ux import _StubViewport,_ctx,_cube,_top
from core.history import History
from tools.pushpull import PushPullTool
root=Path(sys.argv[2]);root.mkdir(parents=True,exist_ok=True)
recipes=json.loads(Path(sys.argv[1]).read_text())['cases']
texture=root/'preview-checker.png';image=QImage(8,8,QImage.Format_RGBA8888)
for y in range(8):
 for x in range(8):image.setPixelColor(x,y,QColor(240 if (x+y)%2 else 20,40,180,255 if x<6 else 0))
image.save(str(texture))
textured=copy.deepcopy(next(r for r in recipes if r['name']=='lone-sheet'));textured['name']='front-texture-back-override';textured['source'][0]['attrs'].update({'texture':{'path':str(texture),'sw':2.,'sh':2.},'back':{'color':[0.,1.,0.],'opacity':.25}});recipes.append(textured)
recipes.append({'name':'ordinary-prism','prism':True,'distance':.6})
gl=ctypes.CDLL('libGL.so.1')
class Observer(Viewport):
 def _draw_preview_faces(self):
  value=ctypes.c_int();gl.glGetIntegerv(0x80a9,ctypes.byref(value));self.preview_samples=value.value
  return super()._draw_preview_faces()
def vec(v):return [v.x(),v.y(),v.z()]
def settle():
 end=time.monotonic()+.3
 while time.monotonic()<end:app.processEvents();time.sleep(.003)
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
rows=[]
for recipe in recipes:
 scene=Scene()
 for f in recipe.get('source',[]):
  face=scene.mesh.add_face([V(*v)for v in f['vertices']],[[V(*v)for v in h]for h in f['holes']]);face.attrs.update(f['attrs']);face.interior=f['interior']
 if recipe.get('prism'):
  _cube(scene,History(scene));face=_top(scene,3.);recipe['face']=scene.mesh.faces.index(face)
  for f in scene.mesh.faces:f.attrs.update({'color':[.3,.5,.7],'back':{'color':[.8,.2,.1],'opacity':.5}})
 else:face=scene.mesh.faces[recipe['face']]
 vp=Observer();vp.setWindowFlags(vp.windowFlags()|Qt.WindowTransparentForInput|Qt.WindowDoesNotAcceptFocus);vp.setFixedSize(800,600);vp.scene=scene
 vp.style_override=Style(face_mode='textures',sky=False,edges=True,profiles=False,background=(1.,1.,1.));scene.display_style=vp.style_override
 vp.camera.target=V(2,2,1.5) if recipe.get('prism') else V(2.5,0,1);vp.camera.distance=8.;vp.camera.yaw=-.75 if recipe.get('prism')else(math.pi/2 if recipe['name']=='back-recess'else -math.pi/2);vp.camera.pitch=.6 if recipe.get('prism')else .25;vp.camera.perspective=False;vp.camera.fov_deg=45.
 stub=_StubViewport(scene,pick=face);tool=PushPullTool();tool.hovered_face=face;tool.on_click(_ctx(stub));vp.active_tool=tool
 vp.show();settle();igz=root/(recipe['name']+'.igz');save_scene(scene,igz)
 captures=[]
 for phase,distance in [('base',0.),('preview',recipe['distance']),('offset',recipe['distance']*1.5),('cancel',0.)]:
  tool.extrusion=distance;tool._show_prism_preview(vp) if recipe.get('prism')else tool._show_light_preview(vp);settle();image=vp.grabFramebuffer();assert image.width()==800 and image.height()==600
  path=root/(recipe['name']+'-'+phase+'.png');image.save(str(path));captures.append({'phase':phase,'distance':distance,'sha256':digest(path),'file':path.name,'suppressed':face in vp._suppressed_faces if hasattr(vp,'_suppressed_faces')else None,'samples':vp.preview_samples,'camera_vp_column_major':list((vp.camera.projection_matrix()*vp.camera.view_matrix()).data())})
 rows.append({'name':recipe['name'],'face':recipe['face'],'normal':vec(tool._normal),'attached':tool._attached,'keep_base':tool._keep_base,'prism_cap':bool(recipe.get('prism')),'anchor':vec(tool._anchor),'camera':{'target':vec(vp.camera.target),'distance':vp.camera.distance,'yaw':vp.camera.yaw,'pitch':vp.camera.pitch,'perspective':False,'fov_deg':45.,'width':800,'height':600},'style':vp.style_override.to_dict(),'igz':igz.name,'igz_sha256':digest(igz),'gl':vp.gl_info,'frames':captures})
 vp.close();vp.deleteLater();app.processEvents()
(root/'original.json').write_text(json.dumps({'source_pin':'6be29fe437a7cd992d4e079f71720821b25216b9','scope':'actual light and ordinary prism preview source passes; application lifecycle separate','texture_sha256':digest(texture),'cases':rows},indent=2)+'\n')
