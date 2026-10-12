#!/usr/bin/env python3
"""One new pinned original physical-picture/import/save/reopen corpus."""
import os,sys,json,hashlib,subprocess,shutil
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
root=Path(__file__).resolve().parents[1];sys.path.insert(0,str(root/'ingetrazo'))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QImage
from core.composition import Composicion,ImagenItem,FormaItem,TextoItem
from views.viewport import Viewport
from views.composer import ComposerWindow
import views.composer as composer
from formats import igz
out=Path(sys.argv[1]).resolve();assert not out.exists(),'Fresh source corpus only';out.mkdir(parents=True);(out/'assets').mkdir()
pin='6be29fe437a7cd992d4e079f71720821b25216b9';assert subprocess.check_output(['git','-C',str(root/'ingetrazo'),'rev-parse','HEAD'],text=True).strip()==pin
assets={'checker.png':'resources/textures/checker.png','person.png':'resources/components/person_billboard.png','beech.jpg':'resources/textures/library/wood/beech.jpg'}
asset_records={}
for name,path in assets.items():
 b=(root/'ingetrazo'/path).read_bytes();(out/'assets'/name).write_bytes(b);asset_records[name]={'original_path':path,'size':len(b),'sha256':hashlib.sha256(b).hexdigest()}
os.chdir(out)
recipes=[('picture-import-plain',dict(imported=True)),('picture-jpeg-stretch',dict(path='assets/beech.jpg')),('picture-cover',dict(fit='cover')),('picture-contain',dict(fit='contain')),('picture-rounded-border',dict(shape='rounded',radius_mm=7.,border=True,border_mm=.45)),('picture-ellipse-feather-opacity',dict(path='assets/person.png',shape='ellipse',feather_mm=2.5,opacity=.63,fit='cover')),('picture-rounded-feather',dict(shape='rounded',radius_mm=5.,feather_mm=1.5)),('picture-opacity-rect',dict(opacity=.37)),('picture-hidden',dict(hidden=True)),('picture-missing',dict(path='assets/missing.png')),('picture-overlap',dict(overlap=True))]
app=QApplication([]);records=[];old=composer.file_dialogs.getOpenFileName
for name,opts in recipes:
 host=QMainWindow();host.viewport=Viewport();scene=host.viewport.scene;sheet=Composicion(name=name,paper='A4',landscape=False);scene.compositions=[sheet];window=ComposerWindow(host)
 if opts.get('imported'):
  composer.file_dialogs.getOpenFileName=lambda *a,**k:('assets/checker.png','');window._on_add_image();composer.file_dialogs.getOpenFileName=old;assert len(sheet.images)==1
 else:
  fields={k:v for k,v in opts.items()if k not in ['overlap','imported']};fields.setdefault('path','assets/checker.png');sheet.images=[ImagenItem(x_mm=30.,y_mm=35.,w_mm=47.,h_mm=33.,**fields)]
 if opts.get('overlap'):
  sheet.images[0].opacity=.63
  sheet.images.append(ImagenItem(x_mm=45.,y_mm=48.,w_mm=31.,h_mm=37.,path='assets/person.png',opacity=.72,z=2.))
  sheet.images.append(ImagenItem(x_mm=39.,y_mm=40.,w_mm=17.,h_mm=13.,path='assets/checker.png',opacity=.44,z=0.))
  sheet.texts=[TextoItem(x_mm=32.,y_mm=45.,w_mm=35.,text='Behind image',size_pt=11.)]
  sheet.shapes=[FormaItem(kind='rect',x_mm=40.,y_mm=42.,w_mm=28.,h_mm=15.,fill=True,fill_color='#d88245',stroke_mm=.35,z=0.)]
 window.export_image(str(out/(name+'.png')),100);window.export_image(str(out/(name+'.jpg')),100);window.export_pdf(str(out/(name+'.pdf')));igz.save_scene(scene,out/(name+'.igz'))
 reopened=type(scene)();igz.load_into(reopened,out/(name+'.igz'));assert [vars(v)for v in reopened.compositions[0].images]==[vars(v)for v in sheet.images]
 images={}
 for extension in ['png','jpg']:
  im=QImage(str(out/(name+'.'+extension))).convertToFormat(QImage.Format_RGB888);assert not im.isNull();raw=bytes(im.constBits());rgb=b''.join(raw[y*im.bytesPerLine():y*im.bytesPerLine()+im.width()*3]for y in range(im.height()));images[extension]={'file':name+'.'+extension,'size':[im.width(),im.height()],'rgb_sha256':hashlib.sha256(rgb).hexdigest()}
 records.append({'case':name,'dpi':100,'scene':name+'.igz','pdf':name+'.pdf','images':images,'recipe':opts,'source_model':[vars(v)for v in sheet.images],'source_reopen':True})
 window.close();host.close()
(out/'source.json').write_text(json.dumps({'source_pin':pin,'assets':asset_records,'cases':records,'scope':'Actual source import, physical PNG/JPEG/PDF, external paths and original IGZ save/reopen; no source rotation field.'},indent=2)+'\n');print(json.dumps({'cases':len(records),'names':[v['case']for v in records]}))
