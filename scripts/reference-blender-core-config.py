import sys,json,os,datetime,tempfile
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core import render_blender as rb
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.layers import Layer
from core.camera import OrbitCamera
from core.sun import ShadowSettings
from georef.datum import SceneDatum
from PySide6.QtGui import QVector3D,QMatrix4x4
cases=json.loads((Path(__file__).resolve().parents[1]/'docs/audits/blender-core-config/cases.json').read_text())
def scene(kind):
 s=Scene()
 if kind=='orphan':s.mesh.vertex(QVector3D(1000,0,-99))
 elif kind in ['loose-edge','hidden-layer']:
  e=s.mesh.add_edge(QVector3D(-25,0,-3),QVector3D(25,0,2));e.hidden=True
  if kind=='hidden-layer':s.layers.append(Layer('Hidden',visible=False));e.layer='Hidden'
 elif kind=='hidden-face':s.mesh.add_face([QVector3D(0,0,-2),QVector3D(40,0,-2),QVector3D(0,50,-2)]).attrs['hidden']=True
 elif kind!='empty':
  m=Mesh();m.add_face([QVector3D(0,0,-4),QVector3D(40,0,-4),QVector3D(0,50,-2)])
  leaf=Group(m);leaf.xform=QMatrix4x4();leaf.xform.translate(-4,8,1);leaf.xform.scale(2,1,.5)
  mid=Group();mid.adopt([leaf]);mid.xform.translate(.125,.25,-3);mid.hidden=kind=='hidden-middle';mid.billboard=kind=='billboard-middle'
  p=Group();p.adopt([mid]);p.xform.translate(5,-8,2);p.hidden=kind=='hidden-root';s.groups=[p]
 return s
jobs=[];realdate=datetime.date
for c in cases['cases']:
 class Date(realdate):
  @classmethod
  def today(cls):return cls(c['year'],10,10)
 datetime.date=Date
 s=scene(c['scene']);s.shadows=ShadowSettings.from_dict(c['shadow'])
 if c['datum'] is not None:s.georef=SceneDatum(**c['datum'])
 cam=OrbitCamera()
 for k,v in c['camera'].items():setattr(cam,k,QVector3D(*v) if k in ['target','up']else v)
 with tempfile.TemporaryDirectory(prefix='ai-blender-source-')as tmp:
  job=json.loads(rb.write_job(s,cam,Path(tmp),engine=c['engine'],quality=c['quality'],width=320,height=240,ambience=c['ambience'],ground=c['ground'],lights=c['lights'],sun_scale=c['sun_scale']).read_text())
  job['glb']='model.glb';job['output']='render.png';jobs.append(dict(name=c['name'],job=job))
datetime.date=realdate
envs=[]
for c in cases['environments']:
 had=hasattr(sys,'frozen');old=getattr(sys,'frozen',None);sys.frozen=c['frozen'];envs.append(rb.clean_env(c['values']))
 if had:sys.frozen=old
 else:del sys.frozen
cache=[];oldhome=os.environ.get('HOME');oldxdg=os.environ.get('XDG_CACHE_HOME');oldflat=rb.in_flatpak
for c in cases['cache']:
 os.environ['HOME']=c['home']
 if c['xdg']is None:os.environ.pop('XDG_CACHE_HOME',None)
 else:os.environ['XDG_CACHE_HOME']=c['xdg']
 rb.in_flatpak=lambda:c['flatpak'];cache.append(str(rb.work_base()))
rb.in_flatpak=oldflat
print(json.dumps(dict(jobs=jobs,environments=envs,cache=cache),indent=2))
