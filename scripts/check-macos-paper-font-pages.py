#!/usr/bin/env python3
"""Strict selected actual helper-painter PDF pages, no broad Text/UI claim."""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
from PySide6.QtWidgets import QApplication
from PySide6.QtPdf import QPdfDocument
from PySide6.QtGui import QImage,QPainter
from PySide6.QtCore import QSize
app=QApplication([]);original=Path(sys.argv[1]);out=Path(sys.argv[2]);dpi=int(sys.argv[3]);names=['arabic','cjk','helvetica','helvetica-bold','helvetica-italic','times-bold-italic'];rows=[]
source=json.loads((original/'original-macos-fonts.json').read_text())
assert source['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9' and source['qt']=='6.12.0'
assert sorted(row['name'] for row in source['cases'])==names
assert sorted(p.name.removesuffix('.native.pdf')for p in out.glob('*.native.pdf'))==names
native=json.loads((out/'native-pdf.json').read_text());assert sorted(row['name'] for row in native)==names
assert all(row['passed'] and row['warm_layout_unchanged'] and row['warm_pdf_bytes_exact'] for row in native)
for name in names:
 images=[];sizes=[]
 for role,path in [('original',original/(name+'.original.pdf')),('native',out/(name+'.native.pdf'))]:
  doc=QPdfDocument();assert doc.load(str(path))==QPdfDocument.Error.None_;assert doc.pageCount()==1;size=doc.pagePointSize(0);sizes.append(size);raw=doc.render(0,QSize(round(size.width()*dpi/72),round(size.height()*dpi/72)));assert not raw.isNull();image=QImage(raw.size(),QImage.Format.Format_RGB32);image.fill(0xffffffff);p=QPainter(image);p.drawImage(0,0,raw);p.end();image=image.convertToFormat(QImage.Format.Format_RGBA8888);image.save(str(out/(name+'.'+role+'.'+str(dpi)+'.png')));images.append(bytes(image.constBits()))
 assert sizes[0]==sizes[1];a,b=images;assert any(a[i:i+3]!=b'\xff\xff\xff'for i in range(0,len(a),4)),name+' source blank'
 changed=sum(a[i:i+4]!=b[i:i+4]for i in range(0,len(a),4));rows.append({'case':name,'changed_pixels':changed,'passed':changed==0})
report={'passed':all(r['passed']for r in rows),'dpi':dpi,'cases':rows,'scope':'Actual six macOS native CoreText production text-painter/direct PDF pages versus frozen original paint_text_mm/QPdfWriter; strict whole-page comparison, actual Canvas GPU pixels/UI remain separate'};(out/('reader-'+str(dpi)+'.json')).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));sys.exit(0 if report['passed']else 1)
