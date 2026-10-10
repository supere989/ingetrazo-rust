#!/usr/bin/env python3
"""Actual source clipboard codec/PasteTool/group history with file-backed textures."""
import os,sys,json,zipfile,hashlib
from pathlib import Path
root=Path(sys.argv[1]).resolve();root.mkdir(parents=True,exist_ok=True)
os.environ['XDG_CACHE_HOME']=str(root/'cache');os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QImage,QColor,QVector3D as V,QMatrix4x4 as M
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from views.viewport import Viewport
from formats import igz,clip
from tools.paste import PasteTool
from acceptance_context import context
app=QApplication([])
image=QImage(2,2,QImage.Format_ARGB32);image.fill(QColor(255,0,0));image.setPixelColor(1,0,QColor(0,255,0));image.setPixelColor(0,1,QColor(0,0,255));image.setPixelColor(1,1,QColor(255,255,255));image_path=root/'checker.png';assert image.save(str(image_path));digest=hashlib.sha256(image_path.read_bytes()).hexdigest()
attrs={'texture':{'path':str(image_path),'sw':1.,'sh':1.,'uvw':[1.,0.,0.,0.,0.,1.,0.,0.]},'mat':'Imported','vendor':{'literal':'preserve'}}
loop=[V(20,0,0),V(21,0,0),V(21,1,0),V(20,1,0)]
source=Scene();face=source.mesh.add_face(loop);face.attrs=dict(attrs);igz.save_scene(source,root/'source-clipboard.igz')
clipboard=clip.decode(clip.encode({'ref':V(20,0,0),'faces':[(loop,[],attrs)],'edges':[],'groups':[]}));assert clipboard
rows={}
def archive(name,scene):
 path=root/(name+'.igz');igz.save_scene(scene,path)
 if zipfile.is_zipfile(path):
  with zipfile.ZipFile(path) as z:return [hashlib.sha256(z.read(n)).hexdigest()for n in z.namelist()if n!='document.json']
 return []
for nested in [False,True]:
 vp=Viewport();vp.flash_status=lambda *a:None
 mesh=Mesh();mesh.add_face([V(),V(1,0,0),V(0,1,0)]);leaf=Group(mesh,'Target');leaf.xform=M();leaf.xform.translate(10,0,0)
 if nested:
  parent=Group(name='Parent');parent.xform=M();parent.xform.translate(100,0,0);parent.children=[leaf];vp.scene.groups=[parent];vp.begin_group_edit(parent)
 else:vp.scene.groups=[leaf]
 vp.begin_group_edit(leaf);vp.clipboard=clipboard;assert PasteTool.in_place(vp);vp.end_one_group_edit()
 if nested:vp.end_one_group_edit()
 label='nested' if nested else 'direct';rows[label]=[]
 for phase in ['finish','undo','redo']:
  if phase=='undo':vp.history.undo()
  if phase=='redo':vp.history.redo()
  hashes=archive(label+'-'+phase,vp.scene)
  textured=[f for f in leaf.mesh.faces if 'texture'in f.attrs]
  rows[label].append({'phase':phase,'faces':len(leaf.mesh.faces),'textured_faces':len(textured),'texture_hashes':hashes,'uvw':[f.attrs['texture']['uvw'] for f in textured],'vendor':[f.attrs.get('vendor')for f in textured]})
assert all(row['texture_hashes']==[digest] for states in rows.values()for row in states if row['phase']!='undo')
report={'provenance':context(),'png_sha256':digest,'cases':rows,'scope':'Actual original clipboard encode/decode, PasteTool stamp and operative group History; no system clipboard or rendered UI mutation.'};(root/'original.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
