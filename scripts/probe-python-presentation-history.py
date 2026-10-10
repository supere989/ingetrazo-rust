#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.viewport import Viewport
from views.tray import ScenesPanel
from core.group import Group
from core.mesh import Mesh
from core.composition import Composicion
from georef.geopath import GeoPath
from georef.terrain import TerrainObject
from core.history import MoveVerticesCommand,MoveGeoPathNodeCommand,SetGeoPathSurfaceCommand,SnapshotImport,StraightenModelCommand
from acceptance_context import context
from core.saved_views import SavedView
from georef.datum import SceneDatum
app=QApplication([]);rows={}
def state(vp):return {'views':[(v.name,v.distance) for v in vp.scene.saved_views],'sheets':[c.name for c in vp.scene.compositions],'terrain':None if vp.scene.terrain is None else vp.scene.terrain.visible,'path':[p.to_dict() for p in vp.scene.geo_paths]}
for name,folded in [('ordinary',False),('folded',True)]:
 vp=Viewport();vp.flash_status=lambda *a:None;m=Mesh();m.add_edge(V(),V(1,0,0));g=Group(m,'Leaf');x=M();x.translate(10,20,30);g.xform=x;vp.scene.groups=[g];vp.scene.compositions=[Composicion(name='Sheet')];vp.scene.terrain=TerrainObject([V(),V(1,0,0),V(0,1,0)],[(0,0),(1,0),(0,1)],[(0,1,2)],(0,0,1,1,0));path=GeoPath([V(2,3,4),V(5,6,7)],name='Route');vp.scene.geo_paths=[path]
 vp.begin_group_edit(g);entry=state(vp)
 if folded:vp.history.execute(MoveVerticesCommand([V(11,20,30)],V(3,0,0)))
 vp.history.execute(MoveGeoPathNodeCommand(path,1,V(8,9,10)));vp.history.execute(SetGeoPathSurfaceCommand([path],'flat'))
 panel=ScenesPanel(SimpleNamespace(viewport=vp,statusBar=lambda:SimpleNamespace(showMessage=lambda *a:None)));panel._on_add();panel.list.setCurrentRow(0);vp.camera.distance=33;panel._on_update();vp.scene.compositions[0].name='Edited sheet';vp.scene.terrain.visible=False
 before=len(vp.history.undo_stack);vp.end_one_group_edit();states=[state(vp)]
 for _ in range(1 if folded else 2):vp.history.undo();states.append(state(vp))
 for _ in range(1 if folded else 2):vp.history.redo();states.append(state(vp))
 rows[name]={'entry':entry,'states':states,'history_before_finish':before}
vp=Viewport();vp.flash_status=lambda *a:None;view=SavedView.capture('Imported',vp.scene,vp.camera);vp.history.execute(SnapshotImport(lambda scene:scene.saved_views.append(view)))
panel=ScenesPanel(SimpleNamespace(viewport=vp,statusBar=lambda:SimpleNamespace(showMessage=lambda *a:None)));panel.list.setCurrentRow(0);panel.list.item(0).setText('Renamed');panel.list.setCurrentRow(0);panel._on_delete();owned=[[(v.name,v.distance) for v in vp.scene.saved_views]];vp.history.undo();owned.append([(v.name,v.distance) for v in vp.scene.saved_views]);vp.history.redo();owned.append([(v.name,v.distance) for v in vp.scene.saved_views]);vp.history.undo();panel._on_add();branch_depth=len(vp.history.redo_stack);vp.history.redo();owned.append([(v.name,v.distance) for v in vp.scene.saved_views]);rows['import_object_lifetime']={'states':owned,'redo_retained_after_ui_add':branch_depth}
vp=Viewport();vp.flash_status=lambda *a:None;m=Mesh();m.add_edge(V(),V(1,0,0));g=Group(m,'Placed');x=M();x.translate(10,20,0);g.xform=x;vp.scene.groups=[g];vp.scene.georef=SceneDatum(37,-122)
def terrain(z):return TerrainObject([V(0,0,z),V(1,0,z),V(0,1,z)],[(0,0),(1,0),(0,1)],[(0,1,2)],(0,0,1,1,0))
def terrain_state():return None if vp.scene.terrain is None else [vp.scene.terrain.vertices[0].z(),vp.scene.terrain.visible]
vp.scene.terrain=terrain(1);vp.history.execute(StraightenModelCommand(g));owned=[terrain_state()];vp.scene.terrain=terrain(9);vp.scene.terrain.visible=False;owned.append(terrain_state());vp.history.undo();owned.append(terrain_state());vp.history.redo();owned.append(terrain_state());vp.scene.terrain=terrain(5);vp.scene.terrain.visible=False;vp.history.undo();owned.append(terrain_state());vp.history.redo();owned.append(terrain_state());rows['terrain_owner_runtime_rebuild']=owned
Path(sys.argv[1]).write_text(json.dumps({'provenance':context(),'cases':rows},indent=2)+'\n')
