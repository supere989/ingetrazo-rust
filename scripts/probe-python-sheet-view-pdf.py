#!/usr/bin/env python3
"""Actual original ComposerWindow orthographic vector page export, no raster substitute."""
import os,sys,json,subprocess
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen");sys.path.insert(0,str(Path("ingetrazo").resolve()))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V
from views.viewport import Viewport
from views.composer import ComposerWindow
from core.composition import Composicion,MarcoVista,FormaItem
from formats import igz
out=Path(os.environ.get("INGETRAZO_VIEW_PDF_OUT","docs/audits/sheet-view-vector-pdf"));out.mkdir(exist_ok=True)
subprocess.run([sys.executable,"scripts/reference-composer-hlr-fixture.py","--original",str(Path("ingetrazo").resolve()),"--output",str(out/"corpus")],check=True)
app=QApplication([])
for path in sorted((out/"corpus").glob("*.reference.json")):
 ref=json.loads(path.read_text());host=QMainWindow();host.viewport=Viewport();vp=host.viewport;igz.load_into(vp.scene,out/"corpus"/(ref["name"]+".igz"))
 for field,value in ref["camera"].items():setattr(vp.camera,field,V(*value)if field in ("target","up")else value)
 scale=float(os.environ.get("INGETRAZO_VIEW_PDF_SCALE","100"))
 hatch=float(os.environ.get("INGETRAZO_VIEW_PDF_HATCH","1.5"))
 fill_color=os.environ.get("INGETRAZO_VIEW_PDF_FILL_COLOR","#595e69")
 if scale!=100:
  from core.composition import ortho_distance_for_height
  vp.camera.distance=ortho_distance_for_height(.1*scale,vp.camera.fov_deg)
  ref["camera"]["distance"]=vp.camera.distance
 ref.update(paper_scale_n=scale,paper_cut_hatch_mm=hatch,paper_cut_fill_color=fill_color)
 if os.environ.get("INGETRAZO_VIEW_PDF_SECTION_TILT")=="1":
  from PySide6.QtGui import QMatrix4x4
  matrix=QMatrix4x4();matrix.rotate(17.,V(1,0,0))
  for plane in vp.scene.section_planes:plane.normal=matrix.mapVector(plane.normal).normalized()
 saved_mode=os.environ.get("INGETRAZO_VIEW_PDF_SAVED")=="1"
 view_key="__current__"
 if saved_mode:
  from core.saved_views import SavedView
  saved=SavedView.capture("Paper saved",vp.scene,vp.camera)
  saved.hidden_shown={"objects":True,"geometry":True}
  vp.scene.saved_views=[saved]
  # Oppose the saved state in the live model. This must not hide the saved
  # paper geometry or replace its cut state; ghosts remain non-paper ink.
  for group in vp.scene.groups:group.hidden=True
  for layer in vp.scene.layers:layer.visible=False
  vp.scene.show_section_cuts=False
  view_key="scene:Paper saved"
  ref["paper_view_key"]=view_key;path.write_text(json.dumps(ref,indent=2))
  igz.save_scene(vp.scene,out/"corpus"/(ref["name"]+".igz"))
 fill_mode=os.environ.get("INGETRAZO_VIEW_PDF_FILL","none")
 grid=float(os.environ.get("INGETRAZO_VIEW_PDF_GRID","0"))
 ref["paper_grid_m"]=grid
 ref["paper_cut_fill"]=fill_mode;path.write_text(json.dumps(ref,indent=2))
 igz.save_scene(vp.scene,out/"corpus"/(ref["name"]+".igz"))
 comps=[]
 from core.hlr import camera_basis
 intermediate=None
 for hidden in (False,True):
  comp=Composicion(name=str(hidden),paper="A4",landscape=False);comp.frames.clear()
  frame=MarcoVista(x_mm=20,y_mm=30,w_mm=100,h_mm=100,view_key=view_key,style="vectorial",scale_n=scale,hidden_lines=hidden,profiles=True,cut_fill=fill_mode,cut_hatch_mm=hatch,cut_fill_color=fill_color,annotations=False,section_marks=False,border=True,border_mm=.43,border_color="#246891",show_title=False,grid_m=grid,pen_edge_mm=.37 if hidden else .18,pen_profile_mm=.62 if hidden else .35,pen_cut_mm=.83 if hidden else .5)
  if intermediate is None:
   window_observer=ComposerWindow(host)
   def observe():
    arrays=vp.hlr_geometry()
    from core.hlr import hlr_drawing,clip_to_section,section_loops
    world_loops=[]
    plane=vp.scene.active_section() if getattr(vp.scene,"show_section_cuts",True) else None
    if plane is not None:
     *_,cuts=clip_to_section(arrays[0].tolist(),[],[],plane,True)
     world_loops=section_loops(cuts)
    drawing=hlr_drawing(vp.scene,vp.camera,geometry=arrays)
    return {"basis":[v.tolist()for v in camera_basis(vp.camera)],"geometry":[v.tolist()for v in arrays],"section_loops":[v.tolist()for v in drawing.loops],"world_section_loops":[v.tolist()for v in world_loops],"camera":{"eye":[vp.camera.eye().x(),vp.camera.eye().y(),vp.camera.eye().z()],"up":[vp.camera.up.x(),vp.camera.up.y(),vp.camera.up.z()]}}
   intermediate=window_observer._with_frame_camera(frame,observe);window_observer.close()
   (out/(ref["name"]+".operative-paper.json")).write_text(json.dumps(intermediate,indent=2))
  comp.frames=[frame];comp.shapes=[FormaItem(kind="rect",x_mm=15,y_mm=25,w_mm=110,h_mm=110,fill=True,fill_color="#99aacc",color="#112233",z=-1),FormaItem(kind="linea",x_mm=20,y_mm=70,w_mm=100,h_mm=0,color="#dd3344",stroke_mm=.8,z=1)];comps.append(comp)
 vp.scene.compositions=comps;window=ComposerWindow(host);assert window.export_all_pdf(str(out/(ref["name"]+".original.pdf")))==[];window.close();host.close()
print("PASS original13scenes×2hidden/penpages PDF exported")
