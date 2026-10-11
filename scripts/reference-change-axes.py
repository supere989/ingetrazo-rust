#!/usr/bin/env python3
"""Pinned original MainWindow Change Axes, actual tool and History; no displayed window."""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QSettings,QPointF,Qt
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from core import extensions
app=QApplication([]);extensions.plugin_dirs=lambda:[];QSettings().setValue('new_document/scale_figure','0')
from views.main_window import MainWindow
from core.scene import Scene
from core.mesh import Mesh
from core.group import Group
from core.history import History
from tools.base import ToolContext
from formats.igz import save_scene,load_into
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True);win=MainWindow();vp=win.viewport

def rows(gs):
 out=[]
 for g in gs:
  out.append({'name':g.name,'xform':None if g.xform is None else list(g.xform.data()),'axes':None if g.axes is None else list(g.axes.data()),'vertices':[list(v.position.toTuple())for v in g.mesh.vertices],'edges':[[g.mesh.vertices.index(e.v0),g.mesh.vertices.index(e.v1)]for e in g.mesh.edges],'faces':[{'outer':[g.mesh.vertices.index(v)for v in f.loop],'holes':[[g.mesh.vertices.index(v)for v in h]for h in f.hole_loops],'attrs':f.attrs}for f in g.mesh.faces],'ext':g.ext})
  out.extend(rows(g.children))
 return out

def state(case,phase):return {'case':case,'phase':phase,'groups':rows(vp.scene.edit_group.children if vp.scene.edit_group is not None else vp.scene.groups),'undo':len(vp.history.undo_stack),'redo':len(vp.history.redo_stack),'selection':[g.name for g in vp.scene.selection if isinstance(g,Group)]}
def mesh():
 m=Mesh();f=m.add_face([V(0,0,0),V(3,0,0),V(3,2,0),V(0,2,0)]);f.attrs={'color':[120,90,60],'vendor':{'keep':[1,2]}};m.add_edge(V(7,0,0),V(7,1,0));m.vertex(V(99,99,99));return m
records=[]
if len(sys.argv)>2 and sys.argv[2]=='inspect':
 result={}
 for path in sorted(out.glob('*.native.igz')):
  s=Scene();load_into(s,path);result[path.stem.removesuffix('.native')]=rows(s.groups)
 print(json.dumps(result));win._saved_version=vp.scene.version;win.close();raise SystemExit
for case in ['classic','shared','nested','oblique','context']:
 s=Scene();g=Group(mesh(),'Target');g.uid='0123456789abcdef';g.ext={'opaque':{'retain':17}}
 if case!='classic':
  g.xform=M();g.xform.translate(2,1,0);sibling=Group(g.mesh,'Sibling');sibling.uid='fedcba9876543210';sibling.xform=M();sibling.xform.translate(10,4,0);sibling.xform.rotate(90,0,0,1)
  if case=='nested':
   child=Group(mesh(),'Child');child.uid='1111111111111111';child.xform=M();child.xform.translate(2,3,1);g.children=[child]
   parent=Group(name='Parent');parent.uid='aaaaaaaaaaaaaaaa';parent.xform=M();parent.xform.translate(4,0,0);parent.children=[sibling];s.groups=[g,parent]
  else:s.groups=[g,sibling]
 else:s.groups=[g]
 if case=='context':
  parent=Group(name='Container');parent.uid='bbbbbbbbbbbbbbbb';parent.xform=M();parent.xform.translate(4,0,0);parent.children=[g];s.groups=[parent,sibling]
 save_scene(s,out/(case+'.input.igz'));s=Scene();load_into(s,out/(case+'.input.igz'));vp.scene=s;vp.history=History(s)
 if case=='context':vp.begin_group_edit(s.groups[0]);g=s.edit_group.children[0]
 else:g=s.groups[0]
 s.selection={g};records.append(state(case,'before'))
 win._on_change_axes(g)
 def click(p):vp.active_tool.on_click(ToolContext(viewport=vp,world=V(*p),screen=QPointF(0,0),snap=None,modifiers=Qt.KeyboardModifier.NoModifier))
 origin=[2,0,0];red=[2,5,0];green=[0,3,0]
 if case=='oblique':origin=[1.1,-0.3,0.7];red=[3.2,1.4,1.1];green=[-0.2,2.3,0.8]
 click(origin);click(origin);click(red);click(red);records.append(state(case,'invalid'))
 click(green);records.append(state(case,'changed'));
 if case!='context':save_scene(s,out/(case+'.source.igz'))
 vp.history.undo();records.append(state(case,'undo'));vp.history.redo();records.append(state(case,'redo'))
 win._on_change_axes(g);click([8,9,0]);vp.active_tool.on_cancel(vp);win._activate_tool('select');records.append(state(case,'cancel'))
 if case=='context':vp.end_group_edit();records.append(state(case,'finish'));save_scene(s,out/(case+'.source.igz'));vp.history.undo();records.append(state(case,'finish-undo'));vp.history.redo();records.append(state(case,'finish-redo'))
(out/'original.json').write_text(json.dumps(records,indent=2));win._saved_version=vp.scene.version;win.close()
