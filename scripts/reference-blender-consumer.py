import json,sys
from pathlib import Path
sys.path.insert(0,'/work/ingetrazo')
from core.scene import Scene
from core.camera import OrbitCamera
from core import render_blender as rb
from PySide6.QtGui import QVector3D
from formats.igz import save_scene
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
s=Scene()
for face in [[(-1,-1,-1),(-1,1,-1),(1,1,-1),(1,-1,-1)],[(-1,-1,1),(1,-1,1),(1,1,1),(-1,1,1)],[(-1,-1,-1),(1,-1,-1),(1,-1,1),(-1,-1,1)],[(1,-1,-1),(1,1,-1),(1,1,1),(1,-1,1)],[(1,1,-1),(-1,1,-1),(-1,1,1),(1,1,1)],[(-1,1,-1),(-1,-1,-1),(-1,-1,1),(-1,1,1)]]:s.mesh.add_face([QVector3D(*p)for p in face])
save_scene(s,out/'input.igz');c=OrbitCamera();c.target=QVector3D(0,0,0);c.distance=5.;c.yaw=c.pitch=0.;c.perspective=True
for engine in ['cycles','eevee']:
 job=rb.write_job(s,c,out/engine,engine=engine,quality=0,width=128,height=96,ground=False,ambience='overcast',lights=[])
 command=rb.command(rb.BlenderFound(['/usr/bin/blender'],'devbox'),job)
 (out/engine/'command.json').write_text(json.dumps(command))
print(json.dumps({'input':str(out/'input.igz'),'original':'6be29fe437a7cd992d4e079f71720821b25216b9'}))
