#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
from types import SimpleNamespace
os.environ["XDG_CONFIG_HOME"]=str(Path(sys.argv[2]).with_suffix(".config"));os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from views.viewport import Viewport
from views.tray import StylesPanel,ShadowsPanel
from core.mesh import Mesh
from core.group import Group
from georef.geopath import GeoPath
from core.history import MoveVerticesCommand,MoveGeoPathNodeCommand
from formats import igz
from acceptance_context import context
app=QApplication([]);rows={}
for nested,folded in [(False,False),(False,True),(True,False),(True,True)]:
 v=Viewport();v.flash_status=lambda *a:None;igz.load_into(v.scene,Path(sys.argv[1])/"input.igz");m=Mesh();m.add_face([V(),V(1,0,0),V(0,1,0)]);g=Group(m,"Leaf");g.component=True;g.xform=M();g.xform.translate(10,0,0)
 if nested:
  parent=Group(name="Parent");parent.children=[g];parent.xform=M();v.scene.groups=[parent];v.begin_group_edit(parent)
 else:v.scene.groups=[g]
 path=GeoPath([V(),V(1,2,3)],name="Route");v.scene.geo_paths=[path];v.begin_group_edit(g)
 def state():return {"style":v.scene.display_style.to_dict(),"shadows":v.scene.shadows.to_dict(),"undo":len(v.history.undo_stack),"redo":len(v.history.redo_stack)}
 entry=state()
 if folded:v.history.execute(MoveVerticesCommand([V(10,0,0)],V(2,0,0)))
 else:v.history.execute(MoveGeoPathNodeCommand(path,1,V(8,9,10)))
 w=SimpleNamespace(viewport=v,_sync_style_menu=lambda:None,statusBar=lambda:SimpleNamespace(showMessage=lambda *a:None));panel=StylesPanel(w);shadow=ShadowsPanel(w);panel._mode.setCurrentIndex(panel._mode.findData("wireframe"));panel._back_edges.setChecked(False);shadow._enable.setChecked(False);v.end_one_group_edit()
 if nested:v.end_one_group_edit()
 states=[state()];v.history.undo();states.append(state());v.history.redo();states.append(state());rows[("nested_" if nested else "")+("folded" if folded else "ordinary")]={"entry":entry,"states":states}
Path(sys.argv[2]).write_text(json.dumps({"provenance":context(),"cases":rows},indent=2)+"\n")
