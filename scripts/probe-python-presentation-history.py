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
from core.history import MoveVerticesCommand,MoveGeoPathNodeCommand,SetGeoPathSurfaceCommand
from acceptance_context import context
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
Path(sys.argv[1]).write_text(json.dumps({'provenance':context(),'cases':rows},indent=2)+'\n')
