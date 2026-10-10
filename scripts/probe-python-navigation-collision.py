#!/usr/bin/env python3
"""Actual frozen MainWindow viewport ray_distance over authored IGZ fixtures."""
import sys,json,tempfile
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ingetrazo'))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D
from views.main_window import MainWindow
from core.scene import Scene
from core.history import History
from core.group import Group
from formats.igz import save_scene
app=QApplication([]);directory=Path(sys.argv[1]);directory.mkdir(parents=True,exist_ok=True);rows=[]
for name in ['loose-face','ordinary-group','billboard-root','hidden-root','hidden-nested-visible-grandchild','billboard-nested-visible-grandchild','billboard-nested-own-face','billboard-root-visible-child']:
 w=MainWindow();vp=w.viewport;vp.scene=Scene();vp.history=History(vp.scene)
 points=[QVector3D(-1,-1,0),QVector3D(1,-1,0),QVector3D(1,1,0),QVector3D(-1,1,0)]
 if name=='loose-face':vp.scene.mesh.add_face(points)
 else:
  root=Group();target=root
  if 'nested' in name:
   child=Group();child.hidden=name.startswith('hidden');child.billboard=name.startswith('billboard');grand=Group();child.children=[grand];root.children=[child];target=child if name=='billboard-nested-own-face' else grand
  if name=='hidden-root':root.hidden=True
  if name.startswith('billboard-root'):root.billboard=True
  if name=='billboard-root-visible-child':
   child=Group();root.children=[child];target=child
  target.mesh.add_face(points);vp.scene.groups.append(root)
 origin=QVector3D(0,0,2);direction=QVector3D(0,0,-1)
 result=vp.ray_distance(origin,direction);save_scene(vp.scene,directory/(name+'.igz'))
 rows.append(dict(name=name,origin=[0,0,2],direction=[0,0,-1],distance=result,placements=len(list(vp.scene.placements()))));w.deleteLater();app.processEvents()
(directory/'original.json').write_text(json.dumps(dict(cases=rows),indent=2)+'\n')
