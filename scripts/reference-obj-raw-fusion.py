#!/usr/bin/env python3
"""Only missing raw-grid versus tolerant-registry and admission-fallback laws."""
import ast,hashlib,json,os,sys
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QMatrix4x4 as M,QVector3D as V,QImage,QColor
from core.mesh import Mesh
from core.texture import texture_file_name
from views.viewport import Viewport
from core.history import SnapshotImport
from formats import obj,igz,dae,fuse
app=QApplication([]);root=Path(sys.argv[1]);root.mkdir(parents=True,exist_ok=True)
module=ast.parse(Path('scripts/reference-nested-import.py').read_text());exec(compile(ast.Module(body=[n for n in module.body if isinstance(n,ast.FunctionDef)and n.name in ['vec','mesh_state','shape']],type_ignores=[]),'accepted-observers','exec'))
module=ast.parse(Path('scripts/reference-obj-fusion.py').read_text());exec(compile(ast.Module(body=[n for n in module.body if isinstance(n,ast.FunctionDef)and n.name=='flags'],type_ignores=[]),'flags-observer','exec'))
module=ast.parse(Path('scripts/reference-obj-mtl-import.py').read_text());exec(compile(ast.Module(body=[n for n in module.body if isinstance(n,ast.FunctionDef)and n.name=='recipe'],type_ignores=[]),'recipe-observer','exec'))
def normalized(v):
 if isinstance(v,dict):
  if 'attrs'in v:v['attrs']=recipe(v['attrs'])
  for k,x in list(v.items()):v[k]=normalized(x)
 elif isinstance(v,list):v=[normalized(x)for x in v]
 return v
image=QImage(2,2,QImage.Format_RGBA8888);image.fill(QColor(37,91,143));assert image.save(str(root/'paint.png'))
(root/'paint.mtl').write_text('newmtl tex\nmap_Kd paint.png\n')
marker='g Marker\nv 5 0 0\nv 6 0 0\nv 5 1 0\nf 5 6 7\n'
cases=[('cross-cell-near-weld','g Patch\nv .000049 0 0\nv 1 0 0\nv 0 1 0\nv .000051 0 0\nv 1 -1 0\nf 1 2 3\nf 2 4 5\ng Marker\nv 5 0 0\nv 6 0 0\nv 5 1 0\nf 6 7 8\n'),('same-cell-far-weld','g Patch\nv -.000049 -.000049 0\nv 1 0 0\nv 0 1 0\nv .000049 .000049 0\nv 1 -1 0\nf 1 2 3\nf 2 4 5\ng Marker\nv 5 0 0\nv 6 0 0\nv 5 1 0\nf 6 7 8\n'),('textured-duplicate-winning-coordinate','mtllib paint.mtl\ng Patch\nv .000049 0 0\nv 1 0 0\nv 0 1 0\nv .000048 0 0\nf 1 2 3\nusemtl tex\nf 4 2 3\nusemtl absent\n'+marker)]
rows=[];inventory=[]
for name,text in cases:
 path=root/(name+'.obj');path.write_text(text);win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp;igz.save_scene(vp.scene,root/(name+'-input.igz'))
 def record(phase):rows.append({'case':name,'phase':phase,'scene':normalized(shape(vp.scene)),'flags':flags(vp.scene),'depth':len(vp.scene._edit_stack),'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack)})
 record('before');vp.history.execute(SnapshotImport(lambda s:obj.load_obj(s,path,scale=1.)));record('import');vp.history.undo();record('undo');vp.history.redo();record('redo');record('finish');igz.save_scene(vp.scene,root/(name+'-original.igz'));inventory.append({'case':name,'file':path.name});win.close()
(root/'original-obj-raw-fusion.json').write_text(json.dumps({'cases':inventory,'states':rows},indent=2)+'\n')
# Explicit dependency fault injection, not a natural malformed geometry claim.
a=[V(0,0,0),V(1,0,0),V(1,1,0)];b=[V(0,0,0),V(1,1,0),V(0,1,0)];attrs={'color':[.2,.3,.4],'opaque':{'preserve':[1]}};controls=[]
for name,loops,refuse in [('fused',[(a,attrs),(b,attrs)],lambda n,k:n>3),('singleton',[(a,attrs)],lambda n,k:k==1),('partial-original',[(a,attrs),(b,attrs)],lambda n,k:k<=2)]:
 mesh=Mesh();original=mesh.add_face;attempts=[]
 def admission(points,holes=None):
  points=list(points);attempts.append(len(points))
  if refuse(len(points),len(attempts)):raise ValueError('explicit injected admission refusal')
  return original(points,holes)
 mesh.add_face=admission;dae._add_fused(mesh,fuse.fuse_coplanar_loops(loops));controls.append({'case':name,'attempts':attempts,'faces':len(mesh.faces),'attrs':[f.attrs for f in mesh.faces],'loops':[[vec(p)for p in f.vertices]for f in mesh.faces]})
(root/'original-admission-fallback.json').write_text(json.dumps(controls,indent=2)+'\n');print(json.dumps({'cases':3,'states':15,'fault_injected_helper_controls':3}))
