#!/usr/bin/env python3
"""One finite actual Qt composer image-authoring handler/widget source capture."""
import argparse,hashlib,json,os,subprocess,sys,shutil,zipfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument("output",type=Path);a=p.parse_args()
out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
for key,name in [("XDG_CONFIG_HOME","config"),("XDG_DATA_HOME","data"),("XDG_CACHE_HOME","cache")]:os.environ[key]=str(out/name)
os.environ["QT_QPA_PLATFORM"]="offscreen"
repo=Path(__file__).resolve().parents[1];original=repo/"ingetrazo"
pin="6be29fe437a7cd992d4e079f71720821b25216b9"
assert subprocess.check_output(["git","-C",str(original),"rev-parse","HEAD"],text=True).strip()==pin
sys.path.insert(0,str(original))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtCore import qVersion
from PySide6.QtGui import QImage
from views.viewport import Viewport
from views.composer import ComposerWindow,ImageItem
import views.composer as composer
from core.composition import Composicion
from formats import igz
app=QApplication([]);app.setOrganizationName("IngeTrazoPictureAuthoringProof");app.setApplicationName("picture-authoring")
assets={"checker.png":"resources/textures/checker.png","person.png":"resources/components/person_billboard.png","beech.jpg":"resources/textures/library/wood/beech.jpg"}
(out/"assets").mkdir();asset_records={}
for name,path in assets.items():
 data=(original/path).read_bytes();(out/"assets"/name).write_bytes(data)
 asset_records[name]=dict(original_path=path,size=len(data),sha256=hashlib.sha256(data).hexdigest())
os.chdir(out)
keys=["frames","texts","images","scalebars","nortes","leyendas","shapes","cotas","cotas_ang","cotas_rad","etiquetas","perfiles","niveles","llamadas"]
all_categories=dict(name="Authoring all-category z",paper="A4",landscape=False,margin_mm=13.5)
for i,key in enumerate(keys):all_categories[key]=[dict(z=float(i+1),hidden=True)]
all_categories["images"][0].update(path="assets/checker.png",locked=True,list_name="Protected picture")
all_categories["leyendas"][0]["z"]=88.;all_categories["perfiles"][0]["z"]=99.
all_categories["cajetin"]=dict(z=55.,hidden=True)
def action(phase,op,**fields):return dict(phase=phase,op=op,**fields)
full=[
 action("initial","observe"),action("add-cancel","add",path=None),action("add-checker","add",path="assets/checker.png"),
 action("select-new","select",index=1),
 *[action(phase,"property",field=field,value=value)for phase,field,value in [
 ("shape-rounded","shape","rounded"),("radius","radius_mm",6.5),("feather","feather_mm",2.5),("fit-cover","fit","cover"),("border-on","border",True),("border-width","border_mm",.45),
 ("opacity63","opacity",63.),("opacity42","opacity",42.)]],
 action("property-noop","property-noop"),action("replace-cancel","replace",path=None),
 action("replace-person","replace",path="assets/person.png"),
 action("undo-replace","undo"),action("redo-replace","redo"),
 action("undo-replace-again","undo"),action("undo-opacity","undo"),action("undo-properties","undo"),action("undo-add","undo"),
 action("redo-add","redo"),action("redo-properties","redo"),action("redo-opacity","redo"),action("redo-replace-final","redo"),
 action("select-final","select",index=1),action("save-reopen","save")]
fallback=[action("initial","observe"),action("add-missing","add",path="assets/missing.png"),action("select","select",index=0),action("replace-jpeg","replace",path="assets/beech.jpg"),action("undo","undo"),action("redo","redo"),action("save-reopen","save")]
aspect=[action("initial","observe"),action("add-person","add",path="assets/person.png"),action("select-first","select",index=0),action("add-checker-while-selected","add",path="assets/checker.png"),action("undo-second","undo"),action("redo-second","redo"),action("replace-same","replace",path="assets/person.png"),action("undo-same","undo"),action("redo-same","redo"),action("save-reopen","save")]
recipes=[("authoring",all_categories,full),("missing-fallback",dict(name="Fallback",paper="A4",landscape=False,margin_mm=9.25),fallback),("aspect-selection",dict(name="Aspect",paper="A4",landscape=False,margin_mm=11.75),aspect)]
states=[];cases=[];reopens=[];guards=[];windows=[];original_chooser=composer.file_dialogs.getOpenFileName
def flush():
 app.processEvents();app.processEvents()
def read_document(path):
 raw=path.read_bytes()
 if raw[:2]==b"PK":
  with zipfile.ZipFile(path)as z:return json.loads(z.read("document.json"))
 return json.loads(raw)
try:
 for name,initial,actions in recipes:
  host=QMainWindow();host.viewport=Viewport()
  host.viewport.scene.compositions=[Composicion.from_dict(initial)]
  inputfile=name+"-input.igz";saved=name+"-original.igz"
  igz.save_scene(host.viewport.scene,out/inputfile)
  # Original writer input plus declared opaque native preservation controls.
  doc=read_document(out/inputfile)
  if name=="authoring":
   comp=doc["scene"]["compositions"][0];comp["vendor_picture_sheet"]={"unicode":"café","flag":True,"value":None}
   comp["images"][0]["vendor_picture"]={"opaque":[1,"one",None,False]}
   (out/inputfile).write_text(json.dumps(doc,ensure_ascii=False,indent=2)+"\n")
   guards.append(dict(case=name,paths={"scene.compositions.0.vendor_picture_sheet":comp["vendor_picture_sheet"],"scene.compositions.0.images.0.vendor_picture":comp["images"][0]["vendor_picture"]},scope="Original reader tolerates and drops these unknown fields; native must retain raw metadata"))
  igz.load_into(host.viewport.scene,out/inputfile)
  window=ComposerWindow(host);windows.append((window,host));window._auto_render=False;window._auto_timer.stop()
  comp=window.comp;created=None
  for step in actions:
   before=comp.to_dict();undo=len(window.history._undo);redo=len(window.history._redo);version=host.viewport.scene.version
   op=step["op"]
   if op in ["add","replace"]:
    path=step["path"]or""
    composer.file_dialogs.getOpenFileName=lambda*a,**k:(path,"")
    if op=="add":
     window._on_add_image()
     if path and created is None:created=comp.images[-1]
    else:window._on_pick_image()
   elif op=="select":
    window.canvas.clearSelection()
    model=comp.images[step["index"]]
    item=next(it for it in window.canvas.items()if isinstance(it,ImageItem)and it.model is model)
    item.force_select();window.on_selection_changed()
   elif op=="property":
    field=step["field"];value=step["value"]
    widget={"shape":window.img_shape,"radius_mm":window.img_radius,"feather_mm":window.img_feather,
     "fit":window.img_fit,"border":window.img_border,"border_mm":window.img_border_mm,"opacity":window.img_opacity}[field]
    if field in ["shape","fit"]:widget.setCurrentIndex(widget.findData(value))
    elif field=="border":widget.setChecked(value)
    else:widget.setValue(value)
   elif op=="property-noop":window._on_image_props()
   elif op=="undo":window._on_undo()
   elif op=="redo":window._on_redo()
   elif op=="save":
    igz.save_scene(host.viewport.scene,out/saved)
    fresh=type(host.viewport.scene)();igz.load_into(fresh,out/saved)
    reopens.append(dict(case=name,composition=fresh.compositions[0].to_dict(),paths=[v.path for v in fresh.compositions[0].images]))
    assert fresh.compositions[0].to_dict()==comp.to_dict()
   elif op!="observe":raise AssertionError(op)
   flush()
   selected=window._selected_item()
   index=next((i for i,m in enumerate(comp.images)if selected is not None and selected.model is m),None)
   reused=None if created is None or not any(m is created for m in comp.images) else True
   states.append(dict(case=name,phase=step["phase"],composition=comp.to_dict(),selection=index,undo=len(window.history._undo),redo=len(window.history._redo),source_version=host.viewport.scene.version,owner_reused=reused))
   if (op in ["add","replace"]and not step["path"])or op in ["observe","select","property-noop","save"]:
    assert comp.to_dict()==before and undo==len(window.history._undo)and redo==len(window.history._redo)and version==host.viewport.scene.version
  cases.append(dict(name=name,input=inputfile,saved=saved,actions=actions))
 result=dict(source_pin=pin,qt=qVersion(),cases=cases,states=states,assets=asset_records,opaque_guards=guards,scope="Actual original ComposerWindow handlers, Qt property signals and original writer/reader; no pointer resize/snaps/multiselection or physical pixel recapture; source_version retained diagnostic")
 (out/"original-picture-authoring.json").write_text(json.dumps(result,ensure_ascii=False,indent=2)+"\n")
 (out/"original-reopens.json").write_text(json.dumps(reopens,ensure_ascii=False,indent=2)+"\n")
 (out/"provenance.json").write_text(json.dumps(dict(source_pin=pin,producer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),source_files={n:hashlib.sha256((original/n).read_bytes()).hexdigest()for n in ["views/composer.py","core/composition.py","formats/igz.py"]}),indent=2)+"\n")
 print(json.dumps(dict(cases=len(cases),states=len(states),reopens=len(reopens))))
finally:
 composer.file_dialogs.getOpenFileName=original_chooser
 for window,host in windows:window.close();host.close()
