#!/usr/bin/env python3
import json,os,sys
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,"/work/ingetrazo")
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QVector3D
from core.scene import Scene
from core.camera import OrbitCamera
from core.saved_views import SavedView
from core.history import History,AddEdgeCommand,HideCommand,DeleteGroupCommand
from formats import igz
app=QApplication.instance() or QApplication([])
base=Path(sys.argv[1]);base.mkdir(parents=True,exist_ok=True)
for case in ["unique","duplicate"]:
 d=base/case;d.mkdir(exist_ok=True)
 I=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
 groups=[{"name":"parent","uid":"parent-16-key","proto":0,"xform":I,"children":[{"name":"child","uid":"child-16-key","proto":0,"xform":I}]},{"name":"sibling","uid":"sibling-16-key","proto":0,"xform":I}]
 if case=="duplicate":groups[0]["uid"]="same-authored-id";groups[1]["uid"]="same-authored-id"
 p=d/"input.igz";p.write_text(json.dumps({"igz_format":1,"app_version":"fixture","scene":{"edges":[],"faces":[],"groups":groups,"protos":[{"edges":[],"faces":[]}],"saved_views":[]}}))
 s=Scene();igz.load_into(s,p);c=OrbitCamera();parent=s.groups[0];child=parent.children[0];sibling=s.groups[1]
 child.hidden=True;sibling.hidden=True;c.distance=37;hidden=SavedView.capture("hidden",s,c)
 child.hidden=False;sibling.hidden=False;c.distance=11;visible=SavedView.capture("visible",s,c)
 s.saved_views=[hidden,visible];igz.save_scene(s,p)
 raw=json.loads(p.read_text());raw["scene"]["saved_views"][0]["vendor_view"]={"retain":[3,4]};p.write_text(json.dumps(raw))
 h=History(s);result=[]
 def rec(label):
  def rows(gs):
   out=[]
   for g in gs:out.append([g.name,g.hidden]);out.extend(rows(g.children))
   return out
  result.append({"label":label,"groups":rows(s.edit_group.children if s.edit_group is not None else s.groups),"distance":c.distance,"target":list(c.target.toTuple()),"edges":len(s.mesh.edges),"vertices":[list(v.position.toTuple()) for v in s.mesh.vertices],"edge_endpoints":[[s.mesh.vertices.index(e.v0),s.mesh.vertices.index(e.v1)] for e in s.mesh.edges],"views":[v.name for v in s.saved_views],"undo":len(h.undo_stack),"redo":len(h.redo_stack)})
 def apply(i):s.saved_views[i].apply(s,c)
 h.execute(AddEdgeCommand(QVector3D(0,0,0),QVector3D(1,0,0)));apply(0);rec("geometry-apply")
 h.undo();rec("geometry-undo");h.redo();rec("geometry-redo")
 c.distance=38;hidden.recapture(s,c);hidden.name="updated";apply(1);rec("update-rename-apply")
 h.undo();rec("updated-geometry-undo");h.redo();rec("updated-geometry-redo")
 h.execute(HideCommand([parent],True));apply(0);rec("hide-then-recall")
 h.undo();rec("hide-undo");h.redo();rec("hide-redo")
 apply(1);h.undo();rec("hide-second-undo");h.redo();rec("hide-second-redo")
 h.execute(HideCommand([parent],True));apply(1);rec("already-hidden-recall")
 h.undo();rec("already-hidden-undo");h.redo();rec("already-hidden-redo")
 apply(0);h.undo();rec("already-hidden-later-recall-undo");h.redo();rec("already-hidden-later-redo")
 h.execute(HideCommand([parent,child,sibling],False));apply(0);rec("unhide-recall")
 h.undo();rec("unhide-undo");h.redo();rec("unhide-redo")
 apply(0);h.undo();rec("unhide-second-undo")
 h.execute(DeleteGroupCommand(parent));apply(1);rec("delete-recall")
 h.undo();rec("delete-undo");h.redo();rec("delete-redo")
 c.distance=90;h.undo();rec("manual-camera-undo");h.redo();rec("manual-camera-redo")
 s.saved_views.remove(visible);h.undo();rec("panel-delete-undo");h.redo();rec("panel-delete-redo")
 h.undo();s.begin_group_edit(parent);s.begin_group_edit(child);h.execute(AddEdgeCommand(QVector3D(2,0,0),QVector3D(3,0,0)));c.target=QVector3D(7,8,9);c.distance=23;rec("nested-context-camera")
 h.undo();rec("nested-context-camera-undo");h.redo();rec("nested-context-camera-redo")
 (d/"original.json").write_text(json.dumps(result,indent=2)+"\n")
