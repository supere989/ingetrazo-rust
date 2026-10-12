#!/usr/bin/env python3
"""Actual pinned SurveyPointsPanel handlers/History; only file and modal answers injected."""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow,QInputDialog
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.viewport import Viewport
from views.tray import SurveyPointsPanel
from views.filedialogs import file_dialogs
from core.group import Group
from core.mesh import Mesh
from georef.datum import SceneDatum
from formats import igz
out=Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True);app=QApplication([])
files={'comma.csv':'P,N,E,Z,D\nA,8660000,300000,1200,"road, north"\nB,8660003,300004,1201,second\n','semicolon.csv':'# survey\nP;N;E;Z;D\nC;8660000,25;300000,5;1202,75;quoted\nD;8660005,25;300008,5;1205,75;last\n','bad.csv':'P,N,E,Z\nno,not,a,point\n'}
for n,t in files.items():(out/n).write_text(t)
rows=[];cases=[]
for name,context,existing in [('fresh','root',False),('existing','root',True),('nested','nested',False)]:
 win=QMainWindow();vp=Viewport();win.setCentralWidget(vp);win.viewport=vp;scene=vp.scene
 scene.units={'length':'ft-in','precision':2}
 mesh=Mesh();mesh.add_face([V(0,0,0),V(1,0,0),V(0,1,0)])
 leaf=Group(mesh,'Leaf');leaf.xform=M();leaf.xform.translate(4,2,1)
 outer=Group(name='Outer');outer.children=[leaf];outer.xform=M();outer.xform.translate(8,1,0);scene.groups=[outer]
 if existing:scene.georef=SceneDatum(-12,-76,1190,north=17)
 igz.save_scene(scene,out/(name+'-input.igz'))
 if context=='nested':vp.begin_group_edit(outer);vp.begin_group_edit(leaf)
 panel=SurveyPointsPanel(win)
 def record(phase):
  rows.append(dict(case=name,phase=phase,points=[p.to_dict() for p in scene.geo_points],datum=scene.georef.to_dict() if scene.georef else None,units=scene.units,undo=len(vp.history.undo_stack),redo=len(vp.history.redo_stack),depth=len(scene._edit_stack),count=panel._count.text(),camera=dict(target=list(vp.camera.target.toTuple()),distance=vp.camera.distance)))
 def invoke(filename,zone_ok=True,hemi_ok=True):
  old=(file_dialogs.getOpenFileName,QInputDialog.getInt,QInputDialog.getItem)
  file_dialogs.getOpenFileName=lambda *a,**k:(str(out/filename),'')
  QInputDialog.getInt=lambda *a,**k:(18,zone_ok)
  QInputDialog.getItem=lambda *a,**k:('South',hemi_ok)
  try:panel._on_import()
  finally:file_dialogs.getOpenFileName,QInputDialog.getInt,QInputDialog.getItem=old
 record('before');invoke('bad.csv');record('bad')
 if not existing:
  invoke('comma.csv',False);record('zone-cancel');invoke('comma.csv',True,False);record('hemisphere-cancel')
 invoke('comma.csv');record('import');vp.history.undo();panel.refresh();record('undo');vp.history.redo();panel.refresh();record('redo')
 invoke('semicolon.csv');record('append');panel._on_clear();record('clear');vp.history.undo();panel.refresh();record('clear-undo');vp.history.redo();panel.refresh();record('clear-redo');panel._on_clear();record('clear-empty');vp.history.undo();panel.refresh();record('restore')
 if context=='nested':vp.end_group_edit();panel.refresh()
 record('finish');igz.save_scene(scene,out/(name+'-original.igz'));vp.history.undo();panel.refresh();record('finish-undo');vp.history.redo();panel.refresh();record('finish-redo')
 cases.append(dict(name=name,context=context,existing=existing));win.close()
(out/'original-survey.json').write_text(json.dumps(dict(cases=cases,states=rows),indent=2)+'\n');print(json.dumps(dict(cases=len(cases),states=len(rows),actual_handlers=True)))
