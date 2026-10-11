#!/usr/bin/env python3
"""Actual pinned Select double-click and DimensionStylePanel handlers; only dialog answers substituted."""
import os,sys,json,math,zipfile
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow,QInputDialog
from PySide6.QtCore import QPointF,Qt
from PySide6.QtGui import QVector3D as V,QColor
import views.tray as tray
from views.viewport import Viewport
from views.tray import DimensionStylePanel
from tools.select import SelectTool,_prompts
from formats import igz
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile('docs/audits/annotation-acquisition/riding-order/source-records.zip') as z:(out/'input.igz').write_bytes(z.read('input.igz'))
app=QApplication([]);rows=[];answers=[]
for kind in ['label','dimension']:
 for context in ['root','nested']:
  win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp;vp.resize(800,600);igz.load_into(vp.scene,out/'input.igz');s=vp.scene
  s.dimension_style['vendor']={'opaque':[1,{'keep':True}]}
  if context=='nested':vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0])
  vp.camera.target=V(0,0,0);vp.camera.distance=10;vp.camera.yaw=0;vp.camera.pitch=math.pi/2;vp.camera.perspective=False;vp.camera.set_aspect(800,600)
  pp=vp._world_to_pixel(s.text_labels[0].position());screen=[pp[0]+8,pp[1]-8] if kind=='label' else list(vp._world_to_pixel(s.dimensions[0].a+s.dimensions[0].offset*1))
  ctx=SimpleNamespace(viewport=vp,screen=QPointF(*screen),world=V(0,0,0),modifiers=Qt.NoModifier);tool=SelectTool()
  def record(phase):
   rows.append(dict(case=kind+'-'+context,phase=phase,screen=screen,labels=[t.text for t in s.text_labels],dimensions=[d.text for d in s.dimensions],style=dict(s.dimension_style),value=vp._format_dim_value(s.dimensions[0].value(),s.dimension_style),undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),selection=len(s.selection),depth=len(s._edit_stack)))
  def answer(text,ok):
   old1=QInputDialog.getMultiLineText;old2=_prompts.get_text
   QInputDialog.getMultiLineText=lambda *a,**k:(text,ok)
   _prompts.get_text=lambda *a,**k:(text,ok)
   try:tool.on_double_click(ctx)
   finally:QInputDialog.getMultiLineText=old1;_prompts.get_text=old2
  record('before');answer(' ignored ',False);record('cancel');answer('  authored\nvalue  ',True);record('edit');answer('authored\nvalue',True);record('noop')
  panel=DimensionStylePanel(win);panel._decimals.setValue(3);panel._units.setCurrentText('cm');panel._font.setValue(17);panel._ends.setCurrentIndex(panel._ends.findData('tick'));panel._base_step.setValue(12.5);old_color=tray.get_color;tray.get_color=lambda *a,**k:QColor(17,42,99)
  try:panel._pick_color()
  finally:tray.get_color=old_color
  record('style')
  vp.history.undo();record('undo');vp.history.redo();record('redo')
  if kind=='dimension':answer(vp._format_dim_value(s.dimensions[0].value(),s.dimension_style),True)
  else:answer('   ',True)
  record('automatic-or-empty');vp.history.undo();record('undo-last');vp.history.redo();record('redo-last')
  if context=='nested':vp.end_group_edit()
  record('finish');igz.save_scene(s,out/(kind+'-'+context+'.igz'))
(out/'original.json').write_text(json.dumps({'states':rows},indent=2))
