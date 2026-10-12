#!/usr/bin/env python3
"""One missing actual pinned physical legend corpus; original product unchanged."""
import os,sys,json,hashlib,subprocess
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
root=Path(__file__).resolve().parents[1];sys.path.insert(0,str(root/'ingetrazo'))
from PySide6.QtWidgets import QApplication,QMainWindow
from PySide6.QtGui import QImage
from core.composition import Composicion,Leyenda,FormaItem,TextoItem
from core.i18n import set_language
from views.viewport import Viewport
from views.composer import ComposerWindow
from formats import igz
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True);pin='6be29fe437a7cd992d4e079f71720821b25216b9'
assert subprocess.check_output(['git','-C',str(root/'ingetrazo'),'rev-parse','HEAD'],text=True).strip()==pin
recipes=[('legend-empty-en',dict(language='en',rows=[])),('legend-empty-es',dict(language='es',rows=[])),('legend-rows',dict(rows=['Roof','Walls','Ground'])),('legend-wrapped',dict(w_mm=36.,title='A long legend heading with wrapping',rows=['A long snapshotted layer name that must wrap inside its own row',''])),('legend-same-z',dict(overlap=True)),('legend-high-z',dict(overlap=True,z=5.)),('legend-hidden',dict(hidden=True))]
app=QApplication([]);rows=[]
for name,opts in recipes:
 if all((out/(name+suffix)).exists()for suffix in ['.png','.pdf','.igz']):
  image=QImage(str(out/(name+'.png'))).convertToFormat(QImage.Format_RGB888);raw=bytes(image.constBits());rgb=b''.join(raw[y*image.bytesPerLine():y*image.bytesPerLine()+image.width()*3]for y in range(image.height()))
  rows.append(dict(case=name,dpi=100,language=opts.get('language','en'),scene=name+'.igz',image=dict(file=name+'.png',size=[image.width(),image.height()],rgb_sha256=hashlib.sha256(rgb).hexdigest()),pdf=name+'.pdf',recipe=opts));continue
 set_language(opts.get('language','en'));host=QMainWindow();host.viewport=Viewport();scene=host.viewport.scene;sheet=Composicion(name=name,paper='A4',landscape=False)
 legend=Leyenda(x_mm=30.,y_mm=35.,w_mm=opts.get('w_mm',65.),title=opts.get('title','LAYERS'),rows=opts.get('rows',['Roof','Walls']),z=opts.get('z',0.),hidden=opts.get('hidden',False));sheet.leyendas=[legend]
 if opts.get('overlap'):
  sheet.leyendas.append(Leyenda(x_mm=48.,y_mm=43.,w_mm=55.,title='SECOND',rows=['Other snapshot'],z=opts.get('z',0.)))
  sheet.texts=[TextoItem(x_mm=30.,y_mm=34.,w_mm=65.,text='Behind legend',size_pt=5.*72./25.4)]
  sheet.shapes=[FormaItem(kind='rect',x_mm=65.,y_mm=44.,w_mm=40.,h_mm=10.,fill=True,fill_color='#d88245',stroke_mm=.35,z=0.)]
 scene.compositions=[sheet];window=ComposerWindow(host);window.export_image(str(out/(name+'.png')),100);window.export_pdf(str(out/(name+'.pdf')));igz.save_scene(scene,out/(name+'.igz'))
 image=QImage(str(out/(name+'.png'))).convertToFormat(QImage.Format_RGB888);raw=bytes(image.constBits());rgb=b''.join(raw[y*image.bytesPerLine():y*image.bytesPerLine()+image.width()*3]for y in range(image.height()))
 rows.append(dict(case=name,dpi=100,language=opts.get('language','en'),scene=name+'.igz',image=dict(file=name+'.png',size=[image.width(),image.height()],rgb_sha256=hashlib.sha256(rgb).hexdigest()),pdf=name+'.pdf',recipe=opts))
 window.close();host.close()
(out/'source.json').write_text(json.dumps(dict(source_pin=pin,images=rows,scope='Actual pinned ComposerWindow physical PNG/PDF exports; layer names are snapshotted strings, distinct z/same-z/hidden/translation branches.'),indent=2)+'\n');print(json.dumps({'cases':len(rows),'names':[r['case']for r in rows]}))
