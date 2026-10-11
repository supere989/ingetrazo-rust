#!/usr/bin/env python3
"""Original MainWindow constructor setup and read-only BIM observations."""
import sys,os,json
from pathlib import Path
sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QTimer
from PySide6.QtGui import QVector3D as V
from views.main_window import MainWindow
app=QApplication([]);win=MainWindow();
from core.group import Group
from core.mesh import Mesh
from PySide6.QtGui import QMatrix4x4 as M
s=win.viewport.scene;f=s.mesh.add_face([V(20,20,0),V(22,20,0),V(22,21,0),V(20,21,0)]);f.attrs["ifc"]={"id":40,"class":"IfcSlab","name":"root"};child=Group(Mesh(),name="target");parent=Group(Mesh(),name="parent");parent.adopt([child]);parent.xform=M();parent.xform.translate(4,5,0);child.xform=M();child.xform.translate(2,0,0);child.mesh.add_face([V(-1,-1,0),V(1,-1,0),V(1,1,0),V(-1,1,0)]);s.groups.append(parent)
win.resize(1200,800);win.viewport.camera.set_view("top");win.viewport.camera.perspective=False;win.viewport.camera.target=V(6,5,0);win.viewport.camera.distance=10.;win.show();win.bim_tray.show();win.bim_tray.raise_();out=Path(os.environ["INGETRAZO_BIM_UI_OUT"])
def xy(p):p=win.mapFromGlobal(p);return [p.x(),p.y()]
def point(p):q=win.viewport._world_to_pixel(V(*p));return xy(win.viewport.mapToGlobal(__import__("PySide6.QtCore",fromlist=["QPoint"]).QPoint(round(q[0]),round(q[1]))))if q else None
def snapshot():
 menu_action=next(a for a in win.menuBar().actions()if a.menu() and win._redo_action in a.menu().actions());menu=menu_action.menu();menu_point=xy(win.menuBar().mapToGlobal(win.menuBar().actionGeometry(menu_action).center()));redo_point=xy(menu.mapToGlobal(menu.actionGeometry(win._redo_action).center()))if menu.isVisible()else None;panel=win.bim_tray.bim;s=win.viewport.scene;r=win.viewport.rect();origin=xy(win.viewport.mapToGlobal(r.topLeft()));state={"name_point":xy(panel.name_edit.mapToGlobal(panel.name_edit.rect().center())),"edit_menu":menu_point,"redo_point":redo_point,"window_id":int(win.winId()),"toggle":xy(panel.active_check.mapToGlobal(__import__("PySide6.QtCore",fromlist=["QPoint"]).QPoint(10,panel.active_check.height()//2))),"active":panel.active_check.isChecked(),"tag":getattr(s,"active_ifc",None),"faces":[{"vertices":[[p.x(),p.y(),p.z()]for p in f.vertices],"attrs":f.attrs}for f in s.mesh.faces],"undo":len(win.viewport.history.undo_stack),"redo":len(win.viewport.history.redo_stack),"tool":type(win.viewport.active_tool).__name__.removesuffix("Tool"),"viewport":[*origin,r.width(),r.height()],"points":[point(p)for p in [(3,2,0),(4,3,0),(7,6,0),(8,7,0),(6,5,0)]],"status":win.statusBar().currentMessage(),"context_depth":len(s._edit_stack)};return state
observe=snapshot
def snapshot():
 state=observe();panel=win.bim_tray.bim
 from PySide6.QtWidgets import QAbstractButton
 csv=next(b for b in panel.findChildren(QAbstractButton)if "CSV"in b.text())
 state["csv_point"]=xy(csv.mapToGlobal(csv.rect().center()))
 file_action=win.menuBar().actions()[0];file_menu=file_action.menu();state["file_menu"]=xy(win.menuBar().mapToGlobal(win.menuBar().actionGeometry(file_action).center()))
 export_action=next(a for a in file_menu.actions()if a.menu()and any("IFC"in b.text()for b in a.menu().actions()))
 state["export_submenu"]=xy(file_menu.mapToGlobal(file_menu.actionGeometry(export_action).center()))if file_menu.isVisible()else None
 export_menu=export_action.menu();ifc_action=next(a for a in export_menu.actions()if "IFC"in a.text())
 state["ifc_menu"]=xy(export_menu.mapToGlobal(export_menu.actionGeometry(ifc_action).center()))if export_menu.isVisible()else None
 (out/"state.tmp").write_text(json.dumps(state));(out/"state.tmp").replace(out/"state.json")
timer=QTimer();timer.timeout.connect(snapshot);timer.start(30);sys.exit(app.exec())
