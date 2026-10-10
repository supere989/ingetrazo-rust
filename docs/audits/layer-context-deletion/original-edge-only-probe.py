#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V,QMatrix4x4 as M
from core.mesh import Mesh
from core.group import Group,copy_group
from core.layers import Layer,assign_layer,layer_of
from views.viewport import Viewport
from views.tray import LayersPanel
from acceptance_context import context
app=QApplication([]);rows={}
for nested,linked in [(False,False),(False,True),(True,False),(True,True)]:
 v=Viewport();v.flash_status=lambda *a:None;v.scene.layers.append(Layer("Tagged"));v.scene.mesh.add_edge(V(10,0,0),V(11,0,0));assign_layer(v.scene.mesh.edges[0],"Tagged");m=Mesh();m.add_edge(V(),V(1,0,0));assign_layer(m.edges[0],"Tagged");g=Group(m,"Inside");g.component=True;g.xform=M();
 if nested:
  parent=Group(name="Parent");parent.children=[g];parent.xform=M();v.scene.groups.append(parent)
 else:v.scene.groups.append(g)
 if linked:v.scene.groups.append(copy_group(g,V(2,0,0)))
 if nested:v.begin_group_edit(parent)
 v.begin_group_edit(g);p=LayersPanel(SimpleNamespace(viewport=v,statusBar=lambda:SimpleNamespace(showMessage=lambda *a:None)));i=next(p.tree.topLevelItem(i) for i in range(p.tree.topLevelItemCount()) if p.tree.topLevelItem(i).text(0)=="Tagged");p.tree.setCurrentItem(i);p._on_delete()
 def state():return {"layers":[l.name for l in v.scene.layers],"outside":layer_of(v.scene.loose_mesh.edges[0]),"inside":layer_of(g.mesh.edges[0]),"sibling":layer_of(v.scene.groups[1].mesh.edges[0]) if linked else None,"undo":len(v.history.undo_stack),"redo":len(v.history.redo_stack)}
 a=[state()];v.end_one_group_edit();
 if nested:v.end_one_group_edit()
 a.append(state());v.history.undo();a.append(state());v.history.redo();a.append(state());rows[("nested_" if nested else "")+("linked" if linked else "single")]=a
Path(sys.argv[1]).write_text(json.dumps({"provenance":context(),"cases":rows},indent=2)+"\n")
