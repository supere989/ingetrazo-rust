#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen');sys.path.insert(0,str(Path('ingetrazo').resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.viewport import Viewport
from core.group import Group
from core.mesh import Mesh
from core.guide import Guide
from core.textlabel import TextLabel
from georef.points import GeoPoint
from georef.datum import SceneDatum
from core.history import AddGuideCommand,AddTextLabelCommand,AddGeoPointsCommand,MoveVerticesCommand
from acceptance_context import context
app=QApplication([])
def state(scene):
 return {'guides':[g.to_dict() for g in scene.guides],'texts':[t.to_dict() for t in scene.text_labels],'points':[p.to_dict() for p in scene.geo_points],'datum':None if scene.georef is None else [scene.georef.lat,scene.georef.lon,scene.georef.alt,scene.georef.north]}
rows={}
for name,folded,nested in [("ordinary",False,False),("folded",True,False),("nested_ordinary",False,True),("nested_folded",True,True)]:
 vp=Viewport();vp.flash_status=lambda *a:None;m=Mesh();m.add_edge(V(),V(1,0,0));g=Group(m,'Leaf');x=M();x.translate(10,20,30);g.xform=x;vp.scene.groups=[g]
 vp.scene.guides=[Guide(V(2,3,4))];vp.scene.text_labels=[TextLabel(V(5,6,7),V(0,2,0),'Existing')];vp.scene.geo_points=[GeoPoint(V(8,9,10),'Existing','Survey')]
 before=state(vp.scene)
 if nested:
  parent=Group(name="Parent");parent.children=[g];vp.scene.groups=[parent];vp.begin_group_edit(parent)
 vp.begin_group_edit(g);entry=state(vp.scene);assert entry==before
 if folded:vp.history.execute(MoveVerticesCommand([V(11,20,30)],V(3,0,0)))
 vp.history.execute(AddGuideCommand(Guide(V(20,21,22))));vp.history.execute(AddTextLabelCommand(TextLabel(V(23,24,25),V(0,3,0),'Added')));vp.history.execute(AddGeoPointsCommand([GeoPoint(V(26,27,28),'Added','Survey')],SceneDatum(37,-122,15,20)))
 vp.end_one_group_edit()
 if nested:vp.end_one_group_edit()
 states=[state(vp.scene)]
 for _ in range(3 if not folded else 1):vp.history.undo();states.append(state(vp.scene))
 for _ in range(3 if not folded else 1):vp.history.redo();states.append(state(vp.scene))
 rows[name]={'entry':entry,'states':states}
Path(sys.argv[1]).write_text(json.dumps({'provenance':context(),'cases':rows},indent=2)+'\n')
