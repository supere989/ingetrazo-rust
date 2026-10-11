#!/usr/bin/env python3
"""Pinned source image writer and operative export-handler controls; no source edits."""
import os,sys,json,hashlib,struct,subprocess
from pathlib import Path
from types import SimpleNamespace,MethodType
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ingetrazo'))
from PySide6.QtWidgets import QApplication,QMainWindow,QInputDialog
from PySide6.QtCore import QSettings,qVersion
from PySide6.QtGui import QImage
from core.composition import Composicion,FormaItem,TextoItem
from core.scene import Scene
from views.composer import ComposerWindow
from views.composer import file_dialogs
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
pin='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(Path(__file__).resolve().parents[1]/'ingetrazo'),'rev-parse','HEAD'],text=True).strip()==pin
app=QApplication([]);app.setOrganizationName('IngeTrazoImageParity');app.setApplicationName('Source')
class Owner(QMainWindow):
 IMAGE_EXPORT_DPI=ComposerWindow.IMAGE_EXPORT_DPI
 _paint_sheet=ComposerWindow._paint_sheet
 _set_field_context=ComposerWindow._set_field_context
 export_image=ComposerWindow.export_image
 _on_export_image=ComposerWindow._on_export_image
 def __init__(self):
  super().__init__();self.comp=Composicion();self.scene=Scene();self.scene.compositions=[self.comp];self._window=SimpleNamespace(_current_path=None);self.refreshes=0
 def _scene(self):return self.scene
 def refresh_all_frames(self):self.refreshes+=1
owner=Owner();rows=[]
def metadata(path):
 blob=path.read_bytes();image=QImage(str(path));assert not image.isNull();rgb=image.convertToFormat(QImage.Format_RGB888);raw=bytes(rgb.constBits());stride=rgb.bytesPerLine();pixels=b''.join(raw[y*stride:y*stride+rgb.width()*3]for y in range(rgb.height()))
 result={'file':path.name,'size':[image.width(),image.height()],'dots_per_meter':[image.dotsPerMeterX(),image.dotsPerMeterY()],'rgb_sha256':hashlib.sha256(pixels).hexdigest(),'file_sha256':hashlib.sha256(blob).hexdigest(),'opaque':not image.hasAlphaChannel()}
 if blob.startswith(b'\x89PNG\r\n\x1a\n'):
  i=8;chunks={}
  while i<len(blob):
   n=struct.unpack('>I',blob[i:i+4])[0];kind=blob[i+4:i+8].decode('ascii');payload=blob[i+8:i+8+n]
   if kind=='pHYs':chunks[kind]=list(struct.unpack('>IIB',payload))
   if kind=='IHDR':chunks[kind]=list(struct.unpack('>IIBBBBB',payload))
   i+=n+12
  result['png']=chunks
 elif blob.startswith(b'\xff\xd8'):
  i=2
  while i<len(blob)and blob[i]==255:
   tag=blob[i+1];n=struct.unpack('>H',blob[i+2:i+4])[0];payload=blob[i+4:i+2+n]
   if tag==224 and payload.startswith(b'JFIF\0'):result['jfif']={'unit':payload[7],'density':list(struct.unpack('>HH',payload[8:12]))}
   if tag==218:break
   i+=2+n
 return result
for name,paper,landscape,dpi,suffix,content in [
 ('blank-portrait','A4',False,50,'png','blank'),('blank-landscape','A4',True,200,'PNG','blank'),('blank-a3','A3',False,72,'jpg','blank'),
 ('shapes','A4',True,100,'png','shapes'),('shapes-jpeg','A4',True,100,'jpeg','shapes'),('shapes-portrait','A4',False,96,'png','shapes'),('text','A4',False,100,'png','text')]:
 c=Composicion(paper=paper,landscape=landscape);owner.comp=c;owner.scene.compositions=[c]
 if content=='shapes':
  c.border=True;c.border_radius_mm=4;c.border_style='double';c.border_color='#80255d9d'
  c.shapes=[FormaItem(kind='rect',x_mm=20,y_mm=25,w_mm=45,h_mm=30,fill=True,fill_color='#80dd6432',radius_mm=5,z=2),FormaItem(kind='elipse',x_mm=40,y_mm=30,w_mm=60,h_mm=45,fill=True,fill_color='#40a0d060',z=1),FormaItem(kind='linea',x_mm=10,y_mm=70,w_mm=80,h_mm=25,color='#255d9d',stroke_mm=.6,z=3),FormaItem(x_mm=1,y_mm=1,w_mm=200,h_mm=200,fill=True,fill_color='#ff0000',hidden=True,z=10)]
 if content=='text':c.texts=[TextoItem(text='IngeTrazo\nPhysical sheet 1:100',x_mm=20,y_mm=30,w_mm=100)]
 path=out/(name+'.'+suffix);owner.export_image(str(path),dpi);rows.append({'case':name,'paper':paper,'landscape':landscape,'dpi':dpi,'content':content,'image':metadata(path)})
# These controls execute the original handler; dialog responses are explicitly supplied.
# Frame refresh is counted, not accepted here as rendering/desktop-dialog proof.
settings=QSettings();settings.clear();controls=[]
for name,path,chosen,dpi,accept in [('path-cancel','','PNG (*.png)',200,True),('resolution-cancel','cancel.png','PNG (*.png)',100,False),('png-extension','chosen','PNG (*.png)',90,True),('jpg-extension','chosen-jpeg','JPEG (*.jpg *.jpeg)',110,True),('uppercase-extension','upper.JPG','PNG (*.png)',120,True),('failure','missing/file.png','PNG (*.png)',130,True)]:
 owner.comp=Composicion();owner.scene.compositions=[owner.comp];owner.refreshes=0;calls=[]
 settings_before=settings.value('composer/image_dpi',None)
 file_dialogs.getSaveFileName=lambda *a,path=path,chosen=chosen:(str(out/path)if path else'',chosen)
 def get_int(*args):calls.append(list(args[3:]));return dpi,accept
 QInputDialog.getInt=get_int
 error=None
 before={p.name for p in out.iterdir()}
 try:owner._on_export_image()
 except OSError:error='OSError'
 added=[metadata(p)for p in out.iterdir()if p.name not in before and p.is_file()]
 controls.append({'case':name,'refreshes':owner.refreshes,'dialog_args':calls,'settings_before':settings_before,'settings_after':settings.value('composer/image_dpi',None),'error':error,'images':added})
record={'source_pin':pin,'qt':qVersion(),'images':rows,'controls':controls,'scope':'Original export_image and _paint_sheet; original _on_export_image with supplied dialog answers and counted refresh. No native render or desktop-dialog acceptance.'}
(out/'source.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'images':len(rows),'handler_controls':len(controls),'qt':qVersion()}))
