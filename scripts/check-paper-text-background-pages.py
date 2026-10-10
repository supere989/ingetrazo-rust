#!/usr/bin/env python3
"""Strict selected actual helper-painter PDF pages, no broad Text/UI claim."""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
from PySide6.QtWidgets import QApplication
from PySide6.QtPdf import QPdfDocument
from PySide6.QtGui import QImage,QPainter
from PySide6.QtCore import QSize
app=QApplication([]);original=Path(sys.argv[1]);out=Path(sys.argv[2]);dpi=int(sys.argv[3]);names=['bg_basic','bg_bold','bg_cjk','bg_empty','bg_italic','bg_rtl','bg_serif','bg_transparent','bg_wrapped'];rows=[]
assert sorted(p.name.removesuffix('.native.pdf')for p in out.glob('*.native.pdf'))==names
for name in names:
 images=[];sizes=[]
 for role,path in [('original',original/(name+'.original.pdf')),('native',out/(name+'.native.pdf'))]:
  doc=QPdfDocument();assert doc.load(str(path))==QPdfDocument.Error.None_;assert doc.pageCount()==1;size=doc.pagePointSize(0);sizes.append(size);raw=doc.render(0,QSize(round(size.width()*dpi/72),round(size.height()*dpi/72)));assert not raw.isNull();image=QImage(raw.size(),QImage.Format.Format_RGB32);image.fill(0xffffffff);p=QPainter(image);p.drawImage(0,0,raw);p.end();image=image.convertToFormat(QImage.Format.Format_RGBA8888);image.save(str(out/(name+'.'+role+'.'+str(dpi)+'.png')));images.append(bytes(image.constBits()))
 assert sizes[0]==sizes[1];a,b=images;assert any(a[i:i+3]!=b'\xff\xff\xff'for i in range(0,len(a),4)),name+' source blank'
 changed=sum(a[i:i+4]!=b[i:i+4]for i in range(0,len(a),4));rows.append({'case':name,'changed_pixels':changed,'passed':changed==0})
# Same-size valid PDF mutation: move both left slab corners by 1mm, preserve xref offsets.
b=(out/'bg_basic.native.pdf').read_bytes();assert b.count(b'-0.500000000')==2
bad=out/'bg_basic.background-mutated.pdf';bad.write_bytes(b.replace(b'-0.500000000',b'-1.500000000'))
mut=QPdfDocument();assert mut.load(str(bad))==QPdfDocument.Error.None_ and mut.pageCount()==1
original_image=QImage(str(out/('bg_basic.original.'+str(dpi)+'.png'))).convertToFormat(QImage.Format.Format_RGBA8888);raw=mut.render(0,original_image.size());assert not raw.isNull()
im=QImage(raw.size(),QImage.Format.Format_RGB32);im.fill(0xffffffff);p=QPainter(im);p.drawImage(0,0,raw);p.end();im=im.convertToFormat(QImage.Format.Format_RGBA8888)
native_bytes=bytes(im.constBits());original_bytes=bytes(original_image.constBits());negative_changed=sum(native_bytes[i:i+4]!=original_bytes[i:i+4]for i in range(0,len(native_bytes),4));assert negative_changed>0
report={'passed':all(r['passed']for r in rows),'dpi':dpi,'cases':rows,'valid_background_mutation_rejected_pixels':negative_changed,'scope':'Nine actual original Text background helper pages; production composer dispatch, not full Text UI/P29'};(out/('reader-'+str(dpi)+'.json')).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));sys.exit(0 if report['passed']else 1)
