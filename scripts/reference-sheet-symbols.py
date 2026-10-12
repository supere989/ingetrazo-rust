#!/usr/bin/env python3
"""Frozen missing physical sheet symbol/title painter corpus, source unchanged."""
import os,sys,json,hashlib,subprocess
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
root=Path(__file__).resolve().parents[1];sys.path.insert(0,str(root/'ingetrazo'))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QImage
from core.composition import Composicion,Cajetin,FlechaNorte,BarraEscala
from views.viewport import Viewport
from views.composer import ComposerWindow
from formats import igz
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
pin='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(root/'ingetrazo'),'rev-parse','HEAD'],text=True).strip()==pin
app=QApplication([]);rows=[]
recipes=[('title-grid',{'layout':'grid','corner':'square'}),('title-banded',{'layout':'banded','corner':'square','fill_color':'#cad8e4'}),('title-minimal',{'layout':'minimal','corner':'square'}),('title-rounded',{'layout':'grid','corner':'rounded','radius_mm':5.,'fill_color':'#cad8e4'}),('title-chamfer-double',{'layout':'banded','corner':'chamfer','radius_mm':5.,'double_border':True,'fill_color':'#cad8e4'}),('title-columns-wrap',{'layout':'grid','columns':2,'label_mm':20.}),('north-zero',{'north':0.}),('north-rotated',{'north':37.}),('scale-default',{'scale':100.,'segments':4}),('scale-fractional',{'scale':250.,'segments':3}),('symbols-overlap',{'all':True}),('symbols-hidden',{'all':True,'hidden':True})]
for name,opts in recipes:
 host=QMainWindow();host.viewport=Viewport();comp=Composicion(name=name,paper='A4',landscape=False)
 if name.startswith('title') or opts.get('all'):
  props={k:v for k,v in opts.items() if k not in ('all','hidden')}
  c=Cajetin(x_mm=20.,y_mm=30.,w_mm=170.,h_mm=65.,campos=[['PROJECT','A long engineering project name with multiple words which must wrap into its assigned cell'],['AUTHOR','Native symbol reference'],['SHEET','{lamina} of {total}'],['SCALE',''],['DATE','2026-10-11']],**props)
  c.hidden=opts.get('hidden',False);comp.cajetin=c
 if 'north' in opts or opts.get('all'):
  n=FlechaNorte(x_mm=35.,y_mm=35.,size_mm=30.,angle_deg=opts.get('north',37.));comp.nortes=[n]
 if 'scale' in opts or opts.get('all'):
  sb=BarraEscala(x_mm=25.,y_mm=80.,scale_n=opts.get('scale',100.),segments=opts.get('segments',4));comp.scalebars=[sb]
 host.viewport.scene.compositions=[comp];window=ComposerWindow(host);window.export_image(str(out/(name+'.png')),100);window.export_pdf(str(out/(name+'.pdf')))
 igz.save_scene(host.viewport.scene,out/(name+'.igz'))
 image=QImage(str(out/(name+'.png'))).convertToFormat(QImage.Format_RGB888);raw=bytes(image.constBits());rgb=b''.join(raw[y*image.bytesPerLine():y*image.bytesPerLine()+image.width()*3]for y in range(image.height()))
 rows.append({'case':name,'dpi':100,'scene':name+'.igz','image':{'file':name+'.png','size':[image.width(),image.height()],'rgb_sha256':hashlib.sha256(rgb).hexdigest()},'pdf':name+'.pdf'})
 window.close();host.close()
(out/'source.json').write_text(json.dumps({'source_pin':pin,'images':rows},indent=2)+'\n')
