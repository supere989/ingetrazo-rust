#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
from types import SimpleNamespace
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from PySide6.QtCore import QPointF,Qt
from core.image_plane import ImagePlane
from core.layers import Layer
from core.mesh import Face
from tools.select import SelectTool
from tools.base import ToolContext
from views.viewport import Viewport
from views.tray import LayersPanel
from acceptance_context import context
app=QApplication([]);rows={}
for q in json.loads(Path(sys.argv[1]).read_text()):
 v=Viewport();v.flash_status=lambda *a:None;v.resize(800,600);v.scene.layers=[]
 for l in q['layers']:v.scene.layers.append(Layer(l['name'],visible=l['visible'],locked=l['locked']))
 for p in q['images']:v.scene.image_planes.append(ImagePlane('missing.png',V(*p['origin']),V(*p['u']),V(*p['v']),name=p['name'],opacity=p['opacity'],layer=p['layer'],locked=p['locked']))
 origin,direction=V(*q['ray_origin']),V(*q['ray_dir']);v._pixel_to_ray=lambda *a:(origin,direction)
 if q['face']:v.scene.mesh.add_face([V(),V(2,0,0),V(0,2,0)])
 for i in q['selected']:v.scene.selection.add(v.scene.image_planes[i])
 if q.get('panel'):
  panel=LayersPanel(SimpleNamespace(viewport=v,statusBar=lambda:SimpleNamespace(showMessage=lambda *a:None)))
  item=panel.tree.topLevelItem(0);item.setCheckState(1 if q['panel']=='hide'else 2,Qt.CheckState.Unchecked if q['panel']=='hide'else Qt.CheckState.Checked)
 tool=SelectTool();v.active_tool=tool
 def name(x):return x.name if isinstance(x,ImagePlane)else 'Face' if isinstance(x,Face)else None
 row={"selected_before_click":sorted(name(x)for x in v.scene.selection),"inference":name(v.image_plane_at(0,0)),"selectable":name(v.pick_image_plane(0,0)),"locked_context":name(v.pick_locked_image_plane(0,0)),"pick":name(tool._pick(v,0,0))}
 if row['inference'] is not None or q['face']:
  point,normal=v._current_work_plane((0,0));row['workplane']={"normal":[normal.x(),normal.y(),normal.z()],"constant":V.dotProduct(point,normal)}
 m=Qt.KeyboardModifier.NoModifier
 if 'shift'in q['modifiers']:m|=Qt.KeyboardModifier.ShiftModifier
 if 'ctrl'in q['modifiers']:m|=Qt.KeyboardModifier.ControlModifier
 tool.on_click(ToolContext(v,V(),QPointF(),m,None));row['selected']=sorted(name(x)for x in v.scene.selection);rows[q['name']]=row
Path(sys.argv[2]).write_text(json.dumps({"provenance":context(),"cases":rows},indent=2)+'\n')
