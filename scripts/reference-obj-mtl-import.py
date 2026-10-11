#!/usr/bin/env python3
"""Missing original OBJ/MTL App Import semantics, captured once; no GPU claim."""
import ast,hashlib,json,os,sys,zipfile
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M,QImage,QColor
from views.viewport import Viewport
from core.scene import Scene
from core.history import SnapshotImport
from formats import obj,igz
from core.texture import texture_file_name
app=QApplication([]);root=Path(sys.argv[1]);root.mkdir(parents=True,exist_ok=True)
image=QImage(2,2,QImage.Format_RGBA8888);image.fill(QColor(37,91,143));assert image.save(str(root/'paint with spaces.png'))
(root/'not-image.bin').write_bytes(b'original imports paths without decoding image bytes')
geometry='v 0 0 0\nv 2 0 0\nv 2 1 0\nv 0 1 0\n'
uv='vt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\n'
cases=[
 ('color','newmtl red\nKd 0.2 0.3 0.4\nd 0.1\nTr 0.8\nKa 1 1 1\nKs 1 1 1\nNs 20\n','usemtl red\nf 1 2 3 4\n'),
 ('cream','newmtl cream\nKd 0.96001 0.94999 0.92501\n','usemtl cream\nf 1 2 3 4\n'),
 ('missing-map','newmtl red\nKd 0.2 0.3 0.4\nmap_Kd C:/gone/missing.png\n','usemtl red\nf 1 2 3 4\n'),
 ('windows-options','newmtl tex\nKd 1 0 0\nmap_Kd -s 2 3 1 -o 4 5 0 -bm 1 C:\\Old Folder\\paint with spaces.png\n','usemtl tex\nf 1/1 2/2 3/3 4/4\n'),
 ('quoted-planar','newmtl tex\nmap_Kd "paint with spaces.png"\n','usemtl tex\nf 1 2 3 4\n'),
 ('partial-uv','newmtl tex\nmap_Kd paint with spaces.png\n','usemtl tex\nf 1/1 2/2 3/99 4/4\n'),
 ('negative-uv','newmtl tex\nmap_Kd paint with spaces.png\n','usemtl tex\nf 1/-4 2/-3 3/-2 4/-1\n'),
 ('unknown-material','newmtl red\nKd 0.2 0.3 0.4\n','usemtl absent\nf 1 2 3 4\n'),
 ('invalid-image','newmtl tex\nmap_Kd not-image.bin\n','usemtl tex\nf 1/1 2/2 3/3 4/4\n'),
 ('missing-sidecar',None,'usemtl red\nf 1 2 3 4\n'),
 ('reset-library','newmtl red\nKd 0.2 0.3 0.4\n','usemtl red\nmtllib absent-second.mtl\nf 1 2 3 4\n'),
]
if '--addendum'in sys.argv or '--nested-resource'in sys.argv:cases=[('multipart','newmtl tex\nmap_Kd paint with spaces.png\n','g Left\nusemtl tex\nf 1/1 2/2 3/3 4/4\ng Right\nf 5/1 6/2 7/3 8/4\n')]
source_seed=ast.parse(Path('scripts/reference-nested-import.py').read_text());exec(compile(ast.Module(body=[n for n in source_seed.body if isinstance(n,ast.FunctionDef)and n.name=='seed'],type_ignores=[]),'seed','exec'))
from core.group import Group
from core.mesh import Mesh
from core.layers import Layer

def recipe(attrs):
 attrs=json.loads(json.dumps(attrs));tex=attrs.get('texture')
 if isinstance(tex,dict)and tex.get('path'):
  path=Path(tex['path']);data=path.read_bytes();tex['path']={'name':texture_file_name(str(path)),'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)}
 return attrs

exec(compile(ast.Module(body=[n for n in source_seed.body if isinstance(n,ast.FunctionDef)and n.name in ['vec','mesh_state','shape']],type_ignores=[]),'source-observation','exec'))
def observe(vp):
 def normalized(value):
  if isinstance(value,dict):
   if 'attrs'in value:value['attrs']=recipe(value['attrs'])
   for key,item in list(value.items()):value[key]=normalized(item)
  elif isinstance(value,list):value=[normalized(v)for v in value]
  return value
 return {'scene':normalized(shape(vp.scene)),'depth':len(vp.scene._edit_stack),'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack)}
rows=[];inventory=[]
for name,mtl,tail in cases:
 for context in (['leaf']if '--nested-resource'in sys.argv else ['root','leaf']if name in ['color','windows-options']else['root']):
  case=name+'-'+context;path=root/(case+'.obj');side=root/(case+'.mtl')
  if mtl is not None:side.write_text(mtl)
  extra='v 4 0 0\nv 6 0 0\nv 6 1 0\nv 4 1 0\n'if name=='multipart'else''
  path.write_text('mtllib '+side.name+'\n'+geometry+extra+uv+tail)
  win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp
  if context=='leaf':seed(vp.scene)
  igz.save_scene(vp.scene,root/(case+'-input.igz'))
  if context=='leaf':vp.begin_group_edit(vp.scene.groups[0]);vp.begin_group_edit(vp.scene.edit_group.children[0])
  def record(phase):rows.append({'case':case,'phase':phase,**observe(vp)})
  record('before');vp.history.execute(SnapshotImport(lambda s:obj.load_obj(s,path,scale=1.)));record('import');vp.history.undo();record('undo');vp.history.redo();record('redo');vp.end_group_edit();record('finish');igz.save_scene(vp.scene,root/(case+'-original.igz'));inventory.append({'case':case,'context':context,'file':path.name});win.close()
# Source path/MTL errors happen during parse before emitting pending faces.
errors=[]
for name,text in ([]if '--addendum'in sys.argv or '--nested-resource'in sys.argv else [('bad-color','newmtl a\nKd nope 0 0\n'),('bad-name','newmtl\n')]):
 (root/(name+'.mtl')).write_text(text);path=root/(name+'.obj');path.write_text('mtllib '+name+'.mtl\n'+geometry+'usemtl a\nf 1 2 3 4\n');s=Scene();before=len(s.mesh.faces)
 try:obj.load_obj(s,path)
 except Exception as e:errors.append({'case':name,'failed':True,'faces_unchanged':len(s.mesh.faces)==before,'type':type(e).__name__})
 else:raise AssertionError('source malformed MTL unexpectedly admitted')
if '--addendum'in sys.argv or '--nested-resource'in sys.argv:
 old=json.loads((root/'original-obj-mtl.json').read_text());inventory=old['cases']+inventory;rows=old['states']+rows;errors=old['errors']
(root/'original-obj-mtl.json').write_text(json.dumps({'cases':inventory,'states':rows,'errors':errors},indent=2)+'\n');print(json.dumps({'cases':len(inventory),'states':len(rows),'malformed_source_controls':len(errors)}))
