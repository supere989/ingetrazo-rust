#!/usr/bin/env python3
"""Actual pinned Qt camera matrix/inverse values, not injected matrices."""
import os,sys,json,math,subprocess,hashlib,struct
from pathlib import Path
os.environ["QT_QPA_PLATFORM"]="offscreen"
source,root,out=map(Path,sys.argv[1:4]);out.mkdir(parents=True,exist_ok=True)
assert subprocess.check_output(["git","-C",str(source),"rev-parse","HEAD"],text=True).strip()=="6be29fe437a7cd992d4e079f71720821b25216b9"
sys.path.insert(0,str(source))
from core.camera import OrbitCamera
from PySide6.QtGui import QVector3D
from PySide6.QtCore import qVersion
assert qVersion().startswith("6.12.")
def base():return dict(target=[0,0,0],distance=20.,yaw=-math.pi/4,pitch=math.pi/6,up=[0,0,1],fov_deg=45.,aspect=4/3,znear=.1,zfar=10000.,perspective=True,two_point=False)
recipes=[]
for name,changes in [("perspective",{}),("ortho",dict(perspective=False)),("two-point",dict(two_point=True)),("top",dict(perspective=False,pitch=math.pi/2)),("near-eye",dict(distance=.021,target=[.1,-.2,.3],yaw=.731,pitch=-.27)),("translated",dict(target=[1234.125,-3456.25,56.75],yaw=.937,pitch=.319,distance=87.3,aspect=1.731))]:
 c=base();c.update(changes);recipes.append((name,c))
real=root/"docs/audits/skp-real-model-nvidia-failure/original/protocol.json"
for row in json.loads(real.read_text())["cases"]:
 c=base();c.update({k:v for k,v in row["camera"].items()if k in c});c["aspect"]=row["camera"]["width"]/row["camera"]["height"];recipes.append((row["name"],c))
rows=[]
for name,c in recipes:
 camera=OrbitCamera()
 for k,v in c.items():setattr(camera,k,QVector3D(*v)if k in["target","up"]else v)
 projection=camera.projection_matrix();view=camera.view_matrix();matrix=projection*view;inv,ok=matrix.inverted();assert ok
 rows.append(dict(name=name,camera=c,projection=list(projection.data()),view=list(view.data()),matrix=list(matrix.data()),inverse=list(inv.data()),eye=list(camera.eye().toTuple()),forward=list(camera.forward().toTuple())))
(out/"original.json").write_text(json.dumps(dict(source_pin="6be29fe437a7cd992d4e079f71720821b25216b9",qt=qVersion(),real_camera_protocol_sha256=hashlib.sha256(real.read_bytes()).hexdigest(),cases=rows),indent=2)+"\n")
print(len(rows),"actual Qt camera snapshots")

# Actual source capture method with controlled GL calls: this isolates only the
# singular-matrix lifecycle branch, not real framebuffer resolve behavior.
import ast,types
viewport=source/'views/viewport.py';tree=ast.parse(viewport.read_text())
method=next(n for n in ast.walk(tree)if isinstance(n,ast.FunctionDef)and n.name=='_capture_depth')
method.decorator_list=[];namespace={k:0 for k in ['GL_READ_FRAMEBUFFER','GL_DRAW_FRAMEBUFFER','GL_DEPTH_BUFFER_BIT','GL_NEAREST','GL_FLOAT']}
exec(compile(ast.fix_missing_locations(ast.Module(body=[method],type_ignores=[])),str(viewport),'exec'),namespace)
class GL:
 def __getattr__(self,name):return lambda *args:None
class FBO:
 def width(self):return 2
 def height(self):return 2
 def handle(self):return 1
camera=OrbitCamera();camera.up=QVector3D()
_,inverse_ok=(camera.projection_matrix()*camera.view_matrix()).inverted()
assert not inverse_ok
sentinel=object();gl=GL();instance=types.SimpleNamespace(camera=camera,_depth_snap=sentinel,_depth_fbo=FBO(),_scene_fbo=FBO(),_gl=gl,context=lambda:types.SimpleNamespace(extraFunctions=lambda:gl))
namespace['_capture_depth'](instance,2,2)
assert instance._depth_snap is sentinel
(out/'singular.json').write_text(json.dumps(dict(source_pin='6be29fe437a7cd992d4e079f71720821b25216b9',inverse_ok=inverse_ok,previous_depth_snapshot_retained=True,scope='actual source AST with controlled GL calls; no GPU resolve proof'),indent=2)+'\n')
