"""Original camera-facing cold job and subsequent persistent-worker requests."""
import sys,json
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.camera import OrbitCamera
from core import render_blender as rb
from formats.igz import load_into
from PySide6.QtGui import QVector3D
input=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir(exist_ok=True)
s=Scene();load_into(s,input);c=OrbitCamera();c.target=QVector3D(0,0,0);c.distance=5;c.yaw=c.pitch=0;c.perspective=True
for engine in ['cycles','eevee']:
 job=rb.write_job(s,c,out/engine,engine=engine,quality=0,width=128,height=96,ground=False,ambience='overcast',lights=[],serve=True)
 requests=[]
 for phase,yaw in [('first',0),('camera',.4),('restored',0)]:
  c.yaw=yaw
  requests.append(dict(camera=rb.camera_dict(c),output=str(out/engine/(phase+'.png')),width=128,height=96,samples=rb.SAMPLES[engine][0]))
 (out/engine/'requests.json').write_text(json.dumps(requests,indent=2)+'\n')
