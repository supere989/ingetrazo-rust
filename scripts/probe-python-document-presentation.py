#!/usr/bin/env python3
"""Operative style/shadows history/save/capture contracts, isolated Qt settings."""
import os,sys,json
from pathlib import Path
from types import SimpleNamespace
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
os.environ["XDG_CONFIG_HOME"]=str(out/"config");os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D as V
from views.viewport import Viewport
from views.tray import StylesPanel,ShadowsPanel,ScenesPanel
from core.style import Style
from core.sun import ShadowSettings
from core.history import MoveVerticesCommand
from formats import igz
from acceptance_context import context
app=QApplication([]);v=Viewport();v.flash_status=lambda *a:None;v.scene.mesh.add_edge(V(),V(1,0,0));v.scene.display_style=Style.from_dict({"name":"Authored","face_mode":"shaded","edges":False,"profiles":False,"back_edges":True,"edge_color":[.123456789,.234567891,.345678912],"front_color":[.812345678,.723456789,.634567891],"back_color":None,"background":[.4,.5,.6],"sky":False,"sky_color":[.7,.8,.9],"ground_color":[.2,.3,.4],"section_fill":False,"section_fill_color":[.25,.35,.45]});v.scene.shadows=ShadowSettings.from_dict({"enabled":True,"month":7,"day":19,"hour":13,"minute":25,"darkness":.63,"utc_offset":-5});igz.save_scene(v.scene,out/"input.igz")
w=SimpleNamespace(viewport=v,_sync_style_menu=lambda:None,_sync_shadow_menu=lambda:None,statusBar=lambda:SimpleNamespace(showMessage=lambda *a:None))
style=StylesPanel(w);shadow=ShadowsPanel(w)
def state():return {"style":v.scene.display_style.to_dict(),"shadows":v.scene.shadows.to_dict(),"undo":len(v.history.undo_stack),"redo":len(v.history.redo_stack)}
rows=[state()];v.history.execute(MoveVerticesCommand([V(1,0,0)],V(2,0,0)));style._mode.setCurrentIndex(style._mode.findData("wireframe"));style._back_edges.setChecked(False);shadow._enable.setChecked(False);rows.append(state());v.history.undo();rows.append(state());v.history.redo();rows.append(state());views=ScenesPanel(w);views._on_add();igz.save_scene(v.scene,out/"edited.igz");fresh=type(v.scene)();igz.load_into(fresh,out/"edited.igz");reopened={"style":fresh.display_style.to_dict(),"shadows":fresh.shadows.to_dict(),"view_style":fresh.saved_views[0].style,"view_shadows":fresh.saved_views[0].shadows};(out/"original.json").write_text(json.dumps({"provenance":context(),"states":rows,"reopened":reopened},indent=2)+"\n")
