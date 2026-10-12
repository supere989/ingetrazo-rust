#!/usr/bin/env python3
"""Four missing actual physical View image recipes; frozen model oracles reused."""
import os,sys,json,hashlib,subprocess
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
root=Path(__file__).resolve().parents[1];sys.path.insert(0,str(root/'ingetrazo'))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QVector3D as V,QImage
from core.composition import Composicion,MarcoVista
from core.saved_views import SavedView
from views.viewport import Viewport
from views.composer import ComposerWindow
from formats import igz
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
pin='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(root/'ingetrazo'),'rev-parse','HEAD'],text=True).strip()==pin
app=QApplication([]);rows=[]
for name,model,dpi,saved,crop,hidden in [('view-current','front_box',100,False,False,False),('view-saved','iso_box',100,True,False,False),('view-cropped','iso_box',100,False,True,False),('view-hidden-thin','iso_box',50,False,False,True)]:
 corpus=root/'docs/audits/sheet-view-vector-pdf/corpus';ref=json.loads((corpus/(model+'.reference.json')).read_text());host=QMainWindow();host.viewport=Viewport();vp=host.viewport;igz.load_into(vp.scene,corpus/(model+'.igz'))
 for field,value in ref['camera'].items():setattr(vp.camera,field,V(*value)if field in ('target','up')else value)
 key='__current__'
 if saved:
  sv=SavedView.capture('Image saved',vp.scene,vp.camera);vp.scene.saved_views=[sv];key='scene:Image saved';vp.camera.yaw=-1.5707963267948966;vp.camera.pitch=0.;
  for group in vp.scene.groups:group.hidden=True
  for layer in vp.scene.layers:layer.visible=False
 comp=Composicion(name=name,paper='A4',landscape=False)
 frame=MarcoVista(x_mm=20,y_mm=30,w_mm=35 if crop else 100,h_mm=100,view_key=key,style='vectorial',scale_n=10 if crop else 100,hidden_lines=hidden,profiles=True,cut_fill='none',annotations=False,section_marks=False,border=True,border_mm=.12 if hidden else .43,border_color='#246891',show_title=False,pen_edge_mm=.18,pen_profile_mm=.35,pen_cut_mm=.5)
 comp.frames=[frame];vp.scene.compositions=[comp];window=ComposerWindow(host);window.refresh_all_frames();window.export_image(str(out/(name+'.png')),dpi)
 igz.save_scene(vp.scene,out/(name+'.igz'))
 cam={field:getattr(vp.camera,field)for field in ref['camera']};cam['target']=[vp.camera.target.x(),vp.camera.target.y(),vp.camera.target.z()];cam['up']=[vp.camera.up.x(),vp.camera.up.y(),vp.camera.up.z()]
 image=QImage(str(out/(name+'.png'))).convertToFormat(QImage.Format_RGB888);raw=bytes(image.constBits());rgb=b''.join(raw[y*image.bytesPerLine():y*image.bytesPerLine()+image.width()*3]for y in range(image.height()))
 rows.append({'case':name,'paper':'A4','landscape':False,'dpi':dpi,'content':'view','scene':name+'.igz','camera':cam,'image':{'file':name+'.png','size':[image.width(),image.height()],'rgb_sha256':hashlib.sha256(rgb).hexdigest()},'reused_model':{'file':model+'.igz','sha256':hashlib.sha256((corpus/(model+'.igz')).read_bytes()).hexdigest()},'hlr_segments':len(window.hlr_cache[id(frame)])})
 window.close();host.close()
(out/'source.json').write_text(json.dumps({'source_pin':pin,'images':rows,'scope':'Actual refresh_all_frames and export_image/_paint_sheet physical View writers; retained model recipes; no desktop capture.'},indent=2)+'\n')
print(json.dumps({'images':len(rows),'cases':[r['case']for r in rows]}))
