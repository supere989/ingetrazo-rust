import sys,json
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.camera import OrbitCamera
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from formats.igz import save_scene,load_into
from formats.gltf import save_glb
out=Path(sys.argv[1]);out.mkdir(exist_ok=True)
s=Scene();m=Mesh();m.add_face([V(-1,0,0),V(1,0,0),V(1,0,3),V(-1,0,3)])
g=Group(m,'collapsed');g.billboard='mesh';g.xform=QMatrix4x4();g.xform.scale(0,1,1);s.groups=[g]
save_scene(s,out/'collapse.igz');loaded=Scene();load_into(loaded,out/'collapse.igz');c=OrbitCamera()
data=dict(target=[0,0,0],distance=5,yaw=0,pitch=0,perspective=True)
c.target=V(0,0,0);c.distance=5;c.yaw=c.pitch=0
(out/'collapse.camera.json').write_text(json.dumps(data))
save_glb(loaded,out/'collapse.glb',face_me=lambda feet:c.eye()-feet)
print(json.dumps(dict(source_exports_degenerate_placement=True)))
