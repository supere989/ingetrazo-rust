#!/usr/bin/env python3
"""Strict selected actual helper-painter PDF pages, no broad Text/UI claim."""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
from PySide6.QtWidgets import QApplication
from PySide6.QtPdf import QPdfDocument
from PySide6.QtGui import QImage,QPainter
from PySide6.QtCore import QSize
app=QApplication([]);original=Path(sys.argv[1]);out=Path(sys.argv[2]);dpi=int(sys.argv[3]);names=['bound','empty_title','first_tie','hidden_bound','main','missing_bound','mixed_placeholders','unicode_path'];rows=[]
if os.environ.get('INGETRAZO_FIELD_CALLER')=='1':
 names=['different_frame_uid','different_text_position','equal_text_only','large_metadata_integer','same_literal_frame_uid']
assert sorted(p.name.removesuffix('.native.pdf')for p in out.glob('*.native.pdf'))==names
for name in names:
 images=[];sizes=[];texts=[]
 for role,path in [('original',original/(name+'.original.pdf')),('native',out/(name+'.native.pdf'))]:
  doc=QPdfDocument();assert doc.load(str(path))==QPdfDocument.Error.None_;assert doc.pageCount()==1;texts.append(doc.getAllText(0).text());size=doc.pagePointSize(0);sizes.append(size);raw=doc.render(0,QSize(round(size.width()*dpi/72),round(size.height()*dpi/72)));assert not raw.isNull();image=QImage(raw.size(),QImage.Format.Format_RGB32);image.fill(0xffffffff);p=QPainter(image);p.drawImage(0,0,raw);p.end();image=image.convertToFormat(QImage.Format.Format_RGBA8888);image.save(str(out/(name+'.'+role+'.'+str(dpi)+'.png')));images.append(bytes(image.constBits()))
 assert sizes[0]==sizes[1] and texts[0]==texts[1],(name,texts);a,b=images;assert any(a[i:i+3]!=b'\xff\xff\xff'for i in range(0,len(a),4)),name+' source blank'
 changed=sum(a[i:i+4]!=b[i:i+4]for i in range(0,len(a),4));rows.append({'case':name,'changed_pixels':changed,'passed':changed==0})
negative=None
if os.environ.get('INGETRAZO_FIELD_CALLER')=='1':
 raw=(out/'equal_text_only.native.pdf').read_bytes();assert b'<0001> <0031>' in raw and b'<0003> <0032>' in raw
 expected=b'0 -92.8125000 Td <0001> Tj';assert raw.count(expected)==1
 path=out/'equal_text_only.wrong-index.pdf';path.write_bytes(raw.replace(expected,b'0 -92.8125000 Td <0003> Tj'))
 doc=QPdfDocument();assert doc.load(str(path))==QPdfDocument.Error.None_ and doc.pageCount()==1
 source=QPdfDocument();assert source.load(str(original/'equal_text_only.original.pdf'))==QPdfDocument.Error.None_
 assert source.getAllText(0).text()=='1/2' and doc.getAllText(0).text()=='2/2'
 image=QImage(str(out/('equal_text_only.original.'+str(dpi)+'.png'))).convertToFormat(QImage.Format.Format_RGBA8888)
 raw=doc.render(0,image.size());canvas=QImage(raw.size(),QImage.Format.Format_RGB32);canvas.fill(0xffffffff);p=QPainter(canvas);p.drawImage(0,0,raw);p.end();canvas=canvas.convertToFormat(QImage.Format.Format_RGBA8888)
 a=bytes(image.constBits());b=bytes(canvas.constBits());changed=sum(a[i:i+4]!=b[i:i+4]for i in range(0,len(a),4));assert changed>0
 negative={'actual_source':'1/2','wrong_physical_index_mutation':'2/2','changed_pixels':changed,'valid_pdf_rejected':True}
report={'passed':all(r['passed']for r in rows),'dpi':dpi,'cases':rows,'wrong_index_mutation':negative,'scope':str(len(names))+' actual source field contexts vs production composer export; caller dataclass equivalence bounded controls, not full Text/UI'};(out/('reader-'+str(dpi)+'.json')).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));sys.exit(0 if report['passed']else 1)
