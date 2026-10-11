#!/usr/bin/env python3
"""Capture only the missing pending Paste/Save transition on the retained input."""
import os, sys, json, zipfile
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
sys.path.insert(0, str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication, QMessageBox
from PySide6.QtCore import QSettings, QPointF, Qt
from PySide6.QtGui import QVector3D as V, QMatrix4x4 as M
from core import extensions
from core.scene import Scene
from core.history import History, MoveVerticesCommand
from formats.igz import load_into
from tools.base import ToolContext
from tools.paste import PasteTool
from core.snap import SnapResult
app=QApplication([]); extensions.plugin_dirs=lambda:[]
QSettings().setValue("new_document/scale_figure", "0")
from views.main_window import MainWindow, file_dialogs
win=MainWindow(); vp=win.viewport
out=Path(sys.argv[1]); out.mkdir(parents=True, exist_ok=True)
def state(s):
 rows=[]
 def visit(g, parent):
  m=parent if g.xform is None else parent*g.xform
  rows.append({"name":g.name,"uid":g.uid,"points":sorted([list(m.map(v.position).toTuple()) for v in g.mesh.vertices]),"ext":g.ext})
  for child in g.children: visit(child,m)
 for g in s.groups: visit(g,M())
 return {"root_points":sorted([list(v.position.toTuple()) for v in s.mesh.vertices]),"groups":rows}
if len(sys.argv)>2 and sys.argv[2]=="inspect":
 rows={}
 for path in sorted(out.glob("*.native.igz")):
  s=Scene();load_into(s,path);rows[path.stem.removesuffix(".native")]=state(s)
 (out/"reopened.json").write_text(json.dumps(rows,indent=2))
 win._saved_version=vp.scene.version;win.close();sys.stdout.flush();os._exit(0)
with zipfile.ZipFile("docs/audits/context-document-save/fixtures-and-logs.zip") as z:
 (out/"input.igz").write_bytes(z.read("fixtures/input.igz"))
def ctx(point):
 v=V(*point);return ToolContext(vp,v,QPointF(),Qt.NoModifier,SnapResult(v,"none"))
def phase(name):
 t=vp.active_tool; pending=isinstance(t,PasteTool) and t._clip is not None
 return {"phase":name,"state":state(vp.scene),"depth":len(vp.scene._edit_stack),"undo":len(vp.history.undo_stack),"redo":len(vp.history.redo_stack),"pending":pending,
 "reference":list(t._clip["ref"].toTuple()) if pending else None,"offset":list(t._offset.toTuple()) if pending else None}
rows=[]
for save in ["save","save_as","save_as_cancel","failed"]:
 for finish in ["continue","cancel"]:
  case=save+"-"+finish
  s=Scene();load_into(s,out/"input.igz");vp.scene=s;vp.history=History(s);win._current_path=None
  vp.begin_group_edit(s.groups[0]);vp.begin_group_edit(s.edit_group.children[0])
  vp.history.execute(MoveVerticesCommand([v.position for v in s.mesh.vertices],V(0,0,2)))
  s.select([s.mesh.faces[0]]);assert vp.copy_selection();win._on_paste();t=vp.active_tool
  assert isinstance(t,PasteTool);t.on_hover(ctx([30,40,50]));clip=t._clip
  reference=list(clip["ref"].toTuple());offset=list(t._offset.toTuple())
  path=out/(case+".original.igz");errors=[];dialogs=[]
  QMessageBox.critical=lambda *args:errors.append(str(args[-1]))
  def choose(*args):
   dialogs.append(len(s._edit_stack));return ("" if save=="save_as_cancel" else str(path),"")
  file_dialogs.getSaveFileName=choose
  if save in ["save","failed"]:
   win._current_path=path if save=="save" else out/"missing/failed.igz";win._on_save()
  else:win._on_save_as()
  assert vp.active_tool is t and t._clip is clip
  phases=[phase("after")];assert phases[0]["reference"]==reference and phases[0]["offset"]==offset
  if finish=="continue":t.on_click(ctx([30,40,50]))
  else:t.on_cancel(vp)
  phases.append(phase(finish))
  if finish=="continue":
   win._on_undo();phases.append(phase("undo"));win._on_redo();phases.append(phase("redo"))
  rows.append({"case":case,"phases":phases,"file_exists":path.exists(),"error":bool(errors),"dialog_depths":dialogs})
(out/"original.json").write_text(json.dumps({"rows":rows},indent=2))
win._saved_version=vp.scene.version;win.close()
print(json.dumps({"workflows":len(rows),"source_pending_owners_preserved":True}))
# The offscreen Qt/OpenGL fixture is not a process-close acceptance test.
# All reports and dialog actions finish above; avoid PySide shutdown finalizers.
sys.stdout.flush();os._exit(0)
