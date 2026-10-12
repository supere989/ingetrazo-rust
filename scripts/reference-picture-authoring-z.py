#!/usr/bin/env python3
"""Only the missing all-category Add/Undo stacking contract; no cohort recapture."""
import os,sys,json,hashlib,subprocess
from pathlib import Path
folder=Path(sys.argv[1]).resolve();repo=Path(__file__).resolve().parents[1]
os.environ["QT_QPA_PLATFORM"]="offscreen"
for key,name in [("XDG_CONFIG_HOME","z-config"),("XDG_DATA_HOME","z-data"),("XDG_CACHE_HOME","z-cache")]:os.environ[key]=str(folder/name)
sys.path.insert(0,str(repo/"ingetrazo"))
from PySide6.QtWidgets import QApplication,QMainWindow
from views.viewport import Viewport
from views.composer import ComposerWindow
import views.composer as composer
from formats import igz
pin="6be29fe437a7cd992d4e079f71720821b25216b9"
assert subprocess.check_output(["git","-C",str(repo/"ingetrazo"),"rev-parse","HEAD"],text=True).strip()==pin
assert not(folder/"original-z-controls.json").exists()
app=QApplication([]);app.setOrganizationName("IngeTrazoPictureAuthoringZProof");app.setApplicationName("picture-authoring-z")
os.chdir(folder);host=QMainWindow();host.viewport=Viewport();igz.load_into(host.viewport.scene,folder/"authoring-input.igz")
window=ComposerWindow(host);window._auto_render=False;window._auto_timer.stop()
keys=["frames","texts","images","scalebars","nortes","leyendas","shapes","cotas","cotas_ang","cotas_rad","etiquetas","perfiles","niveles","llamadas","cajetin"]
old=composer.file_dialogs.getOpenFileName;rows=[]
try:
 for i,key in enumerate(keys):
  comp=window.comp;model=comp.cajetin if key=="cajetin" else getattr(comp,key)[0]
  previous=model.z;model.z=float(200+i);before=comp.to_dict()
  composer.file_dialogs.getOpenFileName=lambda*a,**k:("assets/checker.png","")
  window._on_add_image();app.processEvents();app.processEvents()
  added=comp.images[-1];assert added.z==201+i
  row=dict(category=key,fixture_z=float(200+i),new_image_z=added.z,added_undo=len(window.history._undo),added_redo=len(window.history._redo))
  window._on_undo();app.processEvents();app.processEvents()
  assert comp.to_dict()==before
  row.update(undo_restored=True,undo=len(window.history._undo),redo=len(window.history._redo))
  rows.append(row);model.z=previous
 (folder/"original-z-controls.json").write_text(json.dumps(dict(source_pin=pin,rows=rows,scope="Actual ComposerWindow Add/Undo on15 source all_items categories; independent fixture z setup, no44state/pixel recapture",producer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()),indent=2)+"\n")
 print(json.dumps(dict(categories=len(rows))))
finally:
 composer.file_dialogs.getOpenFileName=old;window.close();host.close()
