#!/usr/bin/env python3
"""Strict nineteen actual helper-painter PDF pages; retain layout failures, no broad UI claim."""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
from PySide6.QtWidgets import QApplication
from PySide6.QtPdf import QPdfDocument
from PySide6.QtGui import QImage,QPainter
from PySide6.QtCore import QSize
app=QApplication([]);original=Path(sys.argv[1]);out=Path(sys.argv[2]);dpi=int(sys.argv[3]);names=['accents','arabic','arabic_run','bold','center','cjk','combining','hebrew','hebrew_run','italic','kerns','latin','mono','newlines','right','serif','serif_bold','underline','wrap'];rows=[]
if os.environ.get('INGETRAZO_TEXT_PARAGRAPHS')=='1':
 names=sorted(names+['mixed_latin_cjk','mixed_cjk_rtl','mixed_scripts','spaces','tabs','nbsp','empty_paragraphs','long_word','mixed_wrapped','rtl_wrapped','fallback_underline','fallback_bold'])
if os.environ.get('INGETRAZO_TEXT_FONT_RUNS')=='1':
 names=['fallback_bold','mixed_cjk_rtl','mixed_latin_cjk','mixed_scripts']
if os.environ.get('INGETRAZO_TEXT_LINEBREAKS')=='1':
 names=sorted(['punctuation','cjk_wrap','soft_hyphen','line_separator','paragraph_separator','punctuation_rtl'])
assert sorted(p.name.removesuffix('.native.pdf')for p in out.glob('*.native.pdf'))==names
for name in names:
 images=[];sizes=[]
 for role,path in [('original',original/(name+'.original.pdf')),('native',out/(name+'.native.pdf'))]:
  doc=QPdfDocument();assert doc.load(str(path))==QPdfDocument.Error.None_;assert doc.pageCount()==1;size=doc.pagePointSize(0);sizes.append(size);raw=doc.render(0,QSize(round(size.width()*dpi/72),round(size.height()*dpi/72)));assert not raw.isNull();image=QImage(raw.size(),QImage.Format.Format_RGB32);image.fill(0xffffffff);p=QPainter(image);p.drawImage(0,0,raw);p.end();image=image.convertToFormat(QImage.Format.Format_RGBA8888);image.save(str(out/(name+'.'+role+'.'+str(dpi)+'.png')));images.append(bytes(image.constBits()))
 assert sizes[0]==sizes[1];a,b=images;assert any(a[i:i+3]!=b'\xff\xff\xff'for i in range(0,len(a),4)),name+' source blank'
 changed=sum(a[i:i+4]!=b[i:i+4]for i in range(0,len(a),4));rows.append({'case':name,'changed_pixels':changed,'passed':changed==0})
report={'passed':all(r['passed']for r in rows),'dpi':dpi,'cases':rows,'scope':str(len(names))+' native font/layout pages vs actual original paint_text_mm helper, not application/UI/fullP29; failed cases mandatory'};(out/('reader-'+str(dpi)+'.json')).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));sys.exit(0 if report['passed']else 1)
