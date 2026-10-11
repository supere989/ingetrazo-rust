#!/usr/bin/env python3
"""Actual pinned TextTool and viewport picks/history; substitute only modal answers."""
import os,sys,json,math,zipfile
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow,QInputDialog
from PySide6.QtCore import QPointF,Qt
from PySide6.QtGui import QVector3D as V,QMatrix4x4
from views.viewport import Viewport
from tools.text import TextTool
from formats import igz
from core.units import bind_scene
from core.group import Group
from core.mesh import Mesh
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile('docs/audits/annotation-acquisition/riding-order/source-records.zip') as z:
 (out/'input.igz').write_bytes(z.read('input.igz'))
app=QApplication([]);rows=[];cases=[]
vec=lambda p:[p.x(),p.y(),p.z()]
for context,kind in [(c,k) for c in ['root','nested'] for k in ['edge','face','coordinates']]+[('closed','face')]:
  if len(sys.argv)>2 and context+'-'+kind!=sys.argv[2]:continue
  win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp;vp.resize(800,600)
  igz.load_into(vp.scene,out/'input.igz');s=vp.scene
  s.units={'length':'cm','precision':3};bind_scene(s)
  if not s.mesh.faces:s.mesh.add_face([V(-2,-2,0),V(2,-2,0),V(2,2,0),V(-2,2,0)])
  if context=='nested' and not s.groups[0].children[0].mesh.faces:s.groups[0].children[0].mesh.add_face([V(-2,-2,0),V(2,-2,0),V(2,2,0),V(-2,2,0)])
  if context=='closed':
   g=Group(Mesh(),'scaled face');g.mesh.add_face([V(-1,-1,0),V(1,-1,0),V(1,1,0),V(-1,1,0)])
   g.xform=QMatrix4x4();g.xform.translate(5,0,0);g.xform.scale(2);s.groups.append(g)
  igz.save_scene(s,out/(context+'-'+kind+'-input.igz'))
  if context=='nested':vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0])
  vp.camera.target=V(0,0,0);vp.camera.distance=10;vp.camera.yaw=0;vp.camera.pitch=math.pi/2;vp.camera.perspective=False;vp.camera.set_aspect(800,600)
  tool=TextTool();tool.on_activate(vp)
  if kind=='edge':e=s.mesh.edges[0];point=(e.a+e.b)*.5
  elif context=='closed':point=V(5,0,0)
  elif kind=='face':point=s.mesh.faces[0].centroid()
  else:point=V(30.125,-20.25,0)
  screen=vp._world_to_pixel(point)
  ctx=lambda p:SimpleNamespace(viewport=vp,screen=QPointF(*screen),world=V(p),modifiers=Qt.NoModifier)
  name=context+'-'+kind
  # Existing label selection is independent of AddTextLabel membership.
  if s.text_labels:s.selection.add(s.text_labels[0])
  def record(phase):
   rows.append(dict(case=name,phase=phase,labels=[dict(anchor=vec(t.anchor),offset=vec(t.offset),text=t.text,layer=getattr(t,'layer',None)) for t in s.text_labels],selection=sorted(s.text_labels.index(t) for t in s.selection if t in s.text_labels),undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),depth=len(s._edit_stack),anchor=vec(tool.anchor) if tool.anchor is not None else None,suggestion=tool._default_text))
  def acquire():tool.on_click(ctx(point));tool.on_hover(ctx(point+V(1,2,0)))
  def answer(text,ok):
   old=QInputDialog.getMultiLineText
   def dialog(*a,**k):record('pending-'+('accept' if ok else 'cancel'));return text,ok
   QInputDialog.getMultiLineText=dialog
   try:tool.on_click(ctx(point+V(1,2,0)))
   finally:QInputDialog.getMultiLineText=old
  record('before');acquire();record('anchor');cases.append(dict(case=name,point=vec(point),screen=list(screen),suggestion=tool._default_text))
  answer('ignored',False);record('cancel');acquire();answer('  authored\nlabel  ',True);record('commit')
  vp.history.undo();record('undo');vp.history.redo();record('redo')
  acquire();answer('  ',True);record('empty')
  if context=='nested':vp.end_group_edit()
  record('finish');igz.save_scene(s,out/(name+'.igz'))
if len(sys.argv)>2:
 prior=json.loads((out/'original.json').read_text());cases=prior['cases']+cases;rows=prior['states']+rows
(out/'original.json').write_text(json.dumps(dict(cases=cases,states=rows),indent=2))
