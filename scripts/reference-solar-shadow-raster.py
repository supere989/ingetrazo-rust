#!/usr/bin/env python3
"""Frozen original viewport shadow oracle; controlled sun, unmodified GPU passes."""
import ctypes, hashlib, json, math, subprocess, sys, time
from pathlib import Path
root=Path(__file__).resolve().parents[1]; source=root/'ingetrazo'; out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
pin='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()==pin
sys.path.insert(0,str(source))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QSurfaceFormat,QVector3D,QMatrix4x4
from PySide6.QtCore import qVersion
fmt=QSurfaceFormat();fmt.setVersion(3,3);fmt.setProfile(QSurfaceFormat.CoreProfile);fmt.setDepthBufferSize(24);fmt.setStencilBufferSize(8);fmt.setSamples(4);QSurfaceFormat.setDefaultFormat(fmt)
app=QApplication([])
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.style import Style
from core.section import SectionPlane
from formats.igz import save_scene
from views.viewport import Viewport
from PIL import Image
import numpy as np
tex=Image.new('RGBA',(32,32),(255,255,255,255))
for y in range(32):
 for x in range(32):
  if 8<=x<24 and 8<=y<24:tex.putpixel((x,y),(255,255,255,0))
tex.save(out/'cutout.png')
gl=ctypes.CDLL('libGL.so.1');gl.glGetString.restype=ctypes.c_char_p
sun=(.8,.4,.45);norm=math.sqrt(sum(x*x for x in sun));sun=tuple(x/norm for x in sun)
class Oracle(Viewport):
 def paintGL(self):
  n=ctypes.c_int();gl.glGetIntegerv(0x80a9,ctypes.byref(n));self.paint_samples=n.value
  n=ctypes.c_int();gl.glGetIntegerv(0x8ca6,ctypes.byref(n));self.paint_draw_fbo=n.value
  return super().paintGL()
 def _sun_dir_now(self):
  return sun if self.scene.shadows.enabled else None
 def _render_shadow_map(self,light_vp):
  self.map_renders=getattr(self,'map_renders',0)+1
  return super()._render_shadow_map(light_vp)
def matrix(m):return list(m.data())
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def settle(v):
 v.update();end=time.monotonic()+.18
 while time.monotonic()<end:app.processEvents();time.sleep(.005)
records=[]
for name,alpha,textured,mode,section,offscreen in [
 ('solid',1.,False,'textures',False,False),('glass-69',.69,False,'textures',False,False),
 ('glass-70',.70,False,'textures',False,False),('cutout',1.,True,'textures',False,False),
 ('hidden-line-cutout',1.,True,'hidden_line',False,False),('missing-texture',1.,'missing','textures',False,False),
 ('section',1.,False,'textures',True,False),('offscreen-instance',1.,False,'textures',False,True)]:
 scene=Scene();scene.shadows.enabled=True;scene.shadows.darkness=.55
 height=4. if offscreen else 1.5
 vertices=[[-1.,-1.,height],[1.,-1.,height],[1.,1.,height],[-1.,1.,height]]
 mesh=Mesh() if offscreen else scene.mesh
 face=mesh.add_face([QVector3D(*v)for v in vertices]);face.attrs.update(color=(.9,.3,.15),opacity=alpha)
 if textured:face.attrs['texture']={'path':str(out/('missing.png' if textured=='missing' else 'cutout.png')),'sw':2.,'sh':2.}
 if offscreen:
  group=Group(mesh,'offscreen-caster');group.xform=QMatrix4x4();scene.groups.append(group)
 if section:scene.section_planes.append(SectionPlane(QVector3D(0,0,0),QVector3D(1,0,0),active=True));scene.show_section_planes=False
 style=Style(face_mode=mode,sky=False,edges=False,profiles=False,background=(.85,.9,.95),section_fill=False);scene.styles=[style];scene.active_style_index=0
 target=[-height*sun[0]/sun[2],-height*sun[1]/sun[2],0.] if offscreen else [-1.2,-.6,0.]
 camera={'target':target,'distance':5. if offscreen else 10.,'yaw':-math.pi/2,'pitch':1.2,'perspective':False,'fov_deg':45.,'width':800,'height':600}
 vp=Oracle();vp.resize(800,600);vp.scene=scene;vp.style_override=style;vp.camera.target=QVector3D(*target);vp.camera.distance=camera['distance'];vp.camera.yaw=camera['yaw'];vp.camera.pitch=camera['pitch'];vp.camera.perspective=False;vp.camera.fov_deg=45.;vp.show();settle(vp)
 vp.makeCurrent();metadata={key:gl.glGetString(val).decode()for key,val in [('vendor',0x1f00),('renderer',0x1f01),('version',0x1f02)]};assert 'NVIDIA' in metadata['vendor'],metadata
 samples=ctypes.c_int();gl.glGetIntegerv(0x80a9,ctypes.byref(samples));vp.doneCurrent()
 rows=[]
 for phase,enabled in [('on',True),('off',False),('restored',True)]:
  scene.shadows.enabled=enabled;settle(vp);im=vp.grabFramebuffer();assert not im.isNull()and(im.width(),im.height())==(800,600)
  path=out/(name+'-'+phase+'.png');assert im.save(str(path));rows.append({'phase':phase,'image':path.name,'sha256':digest(path),'map_renders':vp.map_renders})
 assert rows[0]['sha256']==rows[2]['sha256'],'off/on failed exact source restore'
 vp.makeCurrent();fbo=vp._shadow_fbo;assert fbo is not None and fbo.isValid();previous=ctypes.c_int();gl.glGetIntegerv(0x8069,ctypes.byref(previous));gl.glBindTexture(0x0de1,fbo.texture());sampler={}
 for key,val in [('min_filter',0x2801),('mag_filter',0x2800),('wrap_s',0x2802),('wrap_t',0x2803)]:
  n=ctypes.c_int();gl.glGetTexParameteriv(0x0de1,val,ctypes.byref(n));sampler[key]=n.value
 gl.glBindTexture(0x0de1,previous.value);depth=fbo.toImage();assert depth.size().width()==2048;depthpath=out/(name+'-depth.png');assert depth.save(str(depthpath));vp.doneCurrent()
 lo,hi=scene.bounds();points=np.array(vertices);px,py,visible=vp._project_px(points)
 camera_visible=any(bool(v)and 0<=float(x)<800 and 0<=float(y)<600 for x,y,v in zip(px,py,visible))
 if offscreen:assert not camera_visible,{'px':px.tolist(),'py':py.tolist(),'visible':visible.tolist()}
 igz=out/(name+'.igz');save_scene(scene,igz)
 records.append({'name':name,'camera':camera,'style':style.to_dict(),'sun':sun,'bounds':[lo.toTuple(),hi.toTuple()],'light_vp_column_major':matrix(vp._shadow_vp),'camera_vp_column_major':matrix(vp.camera.projection_matrix()*vp.camera.view_matrix()),'opacity':alpha,'cutout':textured,'section':section,'offscreen_instance':offscreen,'caster_camera_visible':camera_visible,'sampler':sampler,'gl':metadata,'samples':vp.paint_samples,'resolved_samples':samples.value,'draw_fbo':vp.paint_draw_fbo,'images':rows,'depth_image':depthpath.name,'depth_sha256':digest(depthpath),'igz':igz.name,'igz_sha256':digest(igz),'source_map_renders':vp.map_renders})
 vp.close();vp.deleteLater();app.processEvents()
report={'source_pin':pin,'qt':qVersion(),'scope':'Actual original viewport, controlled fixed normalized sun only; unmodified shadow caster/receiver/packed-depth passes. Original IGZ plus source on/off/restore frames and actual queried sampler. Full native parity is a separate acceptance.','cases':records,'source_files':{str(p.relative_to(source)):digest(p)for p in [source/'views/viewport.py',source/'resources/shaders/basic.vert',source/'resources/shaders/basic.frag',source/'resources/shaders/depth.vert',source/'resources/shaders/depth.frag']}}
(out/'original.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'cases':len(records),'renderers':sorted({r['gl']['renderer']for r in records}),'samplers':[r['sampler']for r in records],'map_renders':[r['source_map_renders']for r in records]}))
