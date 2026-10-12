#!/usr/bin/env python3
"""Strict physical symbol images + paired PDF render, no fitted tolerances."""
import json,sys,hashlib,subprocess
from pathlib import Path
from PySide6.QtGui import QImage,QPainter
from PySide6.QtPdf import QPdfDocument
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QSize
import os
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
app=QApplication([])
NAMES=['title-grid','title-banded','title-minimal','title-rounded','title-chamfer-double','title-columns-wrap','north-zero','north-rotated','scale-default','scale-fractional','symbols-overlap','symbols-hidden']
def rgb(path):
 im=QImage(str(path));assert not im.isNull(),path;im=im.convertToFormat(QImage.Format_RGB888);b=bytes(im.constBits());return (im.width(),im.height()),b''.join(b[y*im.bytesPerLine():y*im.bytesPerLine()+im.width()*3]for y in range(im.height()))
def compare(a,b):
 sa,aa=rgb(a);sb,bb=rgb(b);assert sa==sb;return {'different_pixels':sum(aa[i:i+3]!=bb[i:i+3]for i in range(0,len(aa),3)),'max_channel_error':max((abs(x-y)for x,y in zip(aa,bb)),default=0),'source_sha256':hashlib.sha256(aa).hexdigest(),'native_sha256':hashlib.sha256(bb).hexdigest()}
source,native,out=map(Path,sys.argv[1:4]);src=json.loads((source/'source.json').read_text());got=json.loads((native/'native.json').read_text());assert [r['case']for r in src['images']]==NAMES;assert [r['case']for r in got['images']]==NAMES
report=[]
for row,actual in zip(src['images'],got['images']):
 assert actual['nonmutating']is True;assert type(actual['dpi'])is int and actual['dpi']==row['dpi'];file=row['image']['file'];size,b=rgb(source/file);assert list(size)==row['image']['size'];assert hashlib.sha256(b).hexdigest()==row['image']['rgb_sha256']
 image=compare(source/file,native/file)
 # Reuse actual QtPdf reader used by established physical View acceptance.
 import tempfile
 with tempfile.TemporaryDirectory()as tmp:
  paths=[]
  for label,folder in [('source',source),('native',native)]:
   doc=QPdfDocument();assert doc.load(str(folder/row['pdf']))==QPdfDocument.Error.None_;assert doc.pageCount()==1
   size=doc.pagePointSize(0);raw=doc.render(0,QSize(round(size.width()*100/72),round(size.height()*100/72)));assert not raw.isNull()
   im=QImage(raw.size(),QImage.Format_RGB32);im.fill(0xffffffff);painter=QPainter(im);painter.drawImage(0,0,raw);painter.end();file=Path(tmp)/(label+'.png');assert im.save(str(file));paths.append(file)
  pdf=compare(*paths)
 report.append({'case':row['case'],'image':image,'pdf':pdf})
assert got['atomic_refusal']is True
out.write_text(json.dumps({'comparisons':report,'exact_images':sum(r['image']['different_pixels']==0 for r in report),'exact_pdfs':sum(r['pdf']['different_pixels']==0 for r in report),'scope':'12 actual shared PDF/GPU source consumers; source font/platform bound'},indent=2)+'\n')
print(out.read_text());sys.exit(0 if all(r[k]['different_pixels']==0 for r in report for k in ['image','pdf'])else 1)
