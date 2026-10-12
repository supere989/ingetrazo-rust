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
NAMES=['title-layout-left','title-layout-center-above','title-layout-right','title-simple-left','title-simple-right-above','title-bar','title-fields','scale-under-left','scale-under-right-enlargement','scale-inside-bl','scale-inside-br','empty-view','empty-title-scale']
def rgb(path):
 im=QImage(str(path));assert not im.isNull(),path;im=im.convertToFormat(QImage.Format_RGB888);b=bytes(im.constBits());return (im.width(),im.height()),b''.join(b[y*im.bytesPerLine():y*im.bytesPerLine()+im.width()*3]for y in range(im.height()))
def compare(a,b):
 sa,aa=rgb(a);sb,bb=rgb(b);assert sa==sb;return {'different_pixels':sum(aa[i:i+3]!=bb[i:i+3]for i in range(0,len(aa),3)),'max_channel_error':max((abs(x-y)for x,y in zip(aa,bb)),default=0),'source_sha256':hashlib.sha256(aa).hexdigest(),'native_sha256':hashlib.sha256(bb).hexdigest()}
def inventories(src,got):
 assert src['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [r['case']for r in src['images']]==NAMES
 assert [r['case']for r in got['images']]==NAMES
 assert got['atomic_refusal']is True
 assert [r['text']for r in src['advance_observations']['rows']]==['A','B','AB','A\nB','A\rB','A\tB','A\u2028B','A\u2029B','A\u009cB']
 assert [r['text']for r in got['advance_observations']]==[r['text']for r in src['advance_observations']['rows']]
 for a,b in zip(src['advance_observations']['rows'],got['advance_observations']):
  assert type(b['advance_26_6'])is int and b['advance_26_6']==int(a['advance']*64)
 for row,actual in zip(src['images'],got['images']):
  assert actual['nonmutating']is True
  assert type(actual['dpi'])is int and actual['dpi']==row['dpi']
  assert actual['file']==row['image']['file'] and actual['pdf']==row['pdf']
  assert type(actual['pixels'])is list and all(type(n)is int for n in actual['pixels'])and actual['pixels']==row['image']['size']
source,native,out=map(Path,sys.argv[1:4]);src=json.loads((source/'source.json').read_text());got=json.loads((native/'native.json').read_text());inventories(src,got)
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
# Schema mutations are checked independently of any honest baseline pixel failures.
import copy,tempfile
negative=[]
for name,mutate in [
 ('missing-native',lambda a,b:b['images'].pop()),
 ('jointly-empty',lambda a,b:(a.update(images=[]),b.update(images=[]))),
 ('jointly-truncated',lambda a,b:(a['images'].pop(),b['images'].pop())),
 ('duplicate-native',lambda a,b:b['images'].__setitem__(-1,b['images'][0])),
 ('wrong-dpi-type',lambda a,b:b['images'][0].update(dpi='100')),
 ('numeric-nonmutating',lambda a,b:b['images'][0].update(nonmutating=1)),
 ('jointly-truncated-advance',lambda a,b:(a['advance_observations']['rows'].pop(),b['advance_observations'].pop())),
 ('false-atomic',lambda a,b:b.update(atomic_refusal=False))]:
 a=copy.deepcopy(src);b=copy.deepcopy(got);mutate(a,b)
 try:inventories(a,b)
 except(AssertionError,KeyError,TypeError):negative.append(name)
 else:raise AssertionError('Schema corruption accepted:'+name)
# Mutate a separately verified exact image, so other retained residuals cannot reject it.
exact=next((r for r in report if r['image']['different_pixels']==0),None)
assert exact is not None,'No accepted baseline exists for meaningful pixel mutation'
exact_file=next(r['image']['file']for r in src['images']if r['case']==exact['case'])
with tempfile.TemporaryDirectory()as tmp:
 im=QImage(str(native/exact_file));from PySide6.QtGui import QColor
 old=im.pixelColor(0,0);im.setPixelColor(0,0,QColor((old.red()+1)%256,old.green(),old.blue()));path=Path(tmp)/'pixel.png';assert im.save(str(path));assert compare(source/exact_file,path)['different_pixels']==1;negative.append('one-pixel-on-exact-baseline')
 empty=Path(tmp)/'empty.pdf';empty.write_bytes(b'');doc=QPdfDocument();assert doc.load(str(empty))!=QPdfDocument.Error.None_;negative.append('empty-pdf')
out.write_text(json.dumps({'comparisons':report,'exact_images':sum(r['image']['different_pixels']==0 for r in report),'exact_pdfs':sum(r['pdf']['different_pixels']==0 for r in report),'negative_controls':negative,'scope':'13 actual ViewFrame caption/empty PDF/GPU source consumers; source font/platform bound'},indent=2)+'\n')
print(out.read_text());sys.exit(0 if all(r[k]['different_pixels']==0 for r in report for k in ['image','pdf'])else 1)
