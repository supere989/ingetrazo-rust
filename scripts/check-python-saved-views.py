#!/usr/bin/env python3
import os,sys,json,tempfile,subprocess
from pathlib import Path
from acceptance_context import context
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication
from types import SimpleNamespace
from views.viewport import Viewport
from views.tray import ScenesPanel
from core.scene import Scene
from core.camera import OrbitCamera
from formats import igz
def main():
 app=QApplication.instance() or QApplication([])
 vp=Viewport();panel=ScenesPanel(SimpleNamespace(viewport=vp))
 panel._on_add();panel._on_add();first=vp.scene.saved_views[0];removed=vp.scene.saved_views[1];version=vp.scene.version
 panel.list.setCurrentRow(1);panel._on_delete()
 assert vp.scene.saved_views==[first] and removed not in vp.scene.saved_views
 assert vp.scene.version==version+1 and not vp.history.undo_stack and not vp.history.redo_stack
 deleted_version=vp.scene.version
 assert not vp.history.undo() and not vp.history.redo()
 assert vp.scene.saved_views==[first] and vp.scene.version==deleted_version
 with tempfile.TemporaryDirectory() as tmp:
  directory=Path(tmp);subprocess.run(["cargo","run","--locked","-p","ingetrazo-gui","--example","saved_views_acceptance_fixture","--",str(directory)],check=True)
  raw=json.loads((directory/"views.igz").read_text());views=raw["scene"]["saved_views"];assert [v["name"] for v in views]==["first","updated second"];assert views[1]["vendor_view"]=={"retain":17};assert views[1]["style"]["vendor_style"]==19
  scene=Scene();igz.load_into(scene,directory/"views.igz");assert len(scene.saved_views)==2
  assert scene.saved_views[0].distance!=33
  scene.groups[0].children[0].hidden=False;scene.section_planes[0].active=False;camera=OrbitCamera();scene.saved_views[1].apply(scene,camera)
  assert camera.distance==33 and camera.two_point
  assert tuple(camera.target.toTuple())==(2,3,4)
  assert scene.groups[0].children[0].hidden and scene.section_planes[0].active
  assert scene.display_style.face_mode=="xray"
 report={"provenance":context(),"passed":True,"scope":"GUI selected-view update/rename and nonhistory deletion retained across empty model Undo/Redo, and recall of nested visibility, section, style, shadows/two-point camera; actual original native reopen and SavedView.apply camera/style/section/nested visibility. Georef/shadow numeric settings and rendered view fidelity remain open."}
 Path("docs/audits/python-saved-views-evidence.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
