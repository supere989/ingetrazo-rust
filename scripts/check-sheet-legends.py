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
NAMES=['legend-empty-en','legend-empty-es','legend-rows','legend-wrapped','legend-same-z','legend-high-z','legend-hidden']
def rgb(path):
 im=QImage(str(path));assert not im.isNull(),path;im=im.convertToFormat(QImage.Format_RGB888);b=bytes(im.constBits());return (im.width(),im.height()),b''.join(b[y*im.bytesPerLine():y*im.bytesPerLine()+im.width()*3]for y in range(im.height()))
def compare(a,b):
 sa,aa=rgb(a);sb,bb=rgb(b);assert sa==sb;return {'different_pixels':sum(aa[i:i+3]!=bb[i:i+3]for i in range(0,len(aa),3)),'max_channel_error':max((abs(x-y)for x,y in zip(aa,bb)),default=0),'source_sha256':hashlib.sha256(aa).hexdigest(),'native_sha256':hashlib.sha256(bb).hexdigest()}
def inventories(src,got):
 assert src['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [r['case']for r in src['images']]==NAMES
 assert [r['case']for r in got['images']]==NAMES
 assert got['atomic_refusal']is True
 for row,actual in zip(src['images'],got['images']):
  assert actual['nonmutating']is True and actual['snapshot_owner']is True
  assert type(actual['dpi'])is int and actual['dpi']==row['dpi']
  assert actual['file']==row['image']['file'] and actual['pdf']==row['pdf']
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
# Original IGZ reader verifies saved legend order/coordinates/rows, without source recapture.
import sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ingetrazo'))
from core.scene import Scene
from formats import igz
portable=[]
for row,actual in zip(src['images'],got['images']):
 a=Scene();b=Scene();igz.load_into(a,source/row['scene']);igz.load_into(b,native/actual['portable'])
 assert [x.to_dict()if hasattr(x,'to_dict')else vars(x)for x in a.compositions[0].leyendas]==[x.to_dict()if hasattr(x,'to_dict')else vars(x)for x in b.compositions[0].leyendas]
 portable.append(row['case'])
# Schema mutations are checked independently of the honest two-pixel baseline failure.
import copy,tempfile
negative=[]
for name,mutate in [
 ('missing-native',lambda a,b:b['images'].pop()),
 ('jointly-empty',lambda a,b:(a.update(images=[]),b.update(images=[]))),
 ('jointly-truncated',lambda a,b:(a['images'].pop(),b['images'].pop())),
 ('duplicate-native',lambda a,b:b['images'].__setitem__(-1,b['images'][0])),
 ('wrong-dpi-type',lambda a,b:b['images'][0].update(dpi='100')),
 ('numeric-nonmutating',lambda a,b:b['images'][0].update(nonmutating=1)),
 ('false-snapshot-owner',lambda a,b:b['images'][0].update(snapshot_owner=False)),
 ('false-atomic',lambda a,b:b.update(atomic_refusal=False))]:
 a=copy.deepcopy(src);b=copy.deepcopy(got);mutate(a,b)
 try:inventories(a,b)
 except(AssertionError,KeyError,TypeError):negative.append(name)
 else:raise AssertionError('Schema corruption accepted:'+name)
# Mutate a separately verified exact image, so other retained residuals cannot reject it.
assert report[0]['case']=='legend-empty-en'and report[0]['image']['different_pixels']==0
with tempfile.TemporaryDirectory()as tmp:
 im=QImage(str(native/'legend-empty-en.png'));from PySide6.QtGui import QColor
 old=im.pixelColor(0,0);im.setPixelColor(0,0,QColor(0,old.green(),old.blue()));path=Path(tmp)/'pixel.png';assert im.save(str(path));assert compare(source/'legend-empty-en.png',path)['different_pixels']==1;negative.append('one-pixel-on-exact-baseline')
 empty=Path(tmp)/'empty.pdf';empty.write_bytes(b'');doc=QPdfDocument();assert doc.load(str(empty))!=QPdfDocument.Error.None_;negative.append('empty-pdf')
out.write_text(json.dumps({'comparisons':report,'exact_images':sum(r['image']['different_pixels']==0 for r in report),'exact_pdfs':sum(r['pdf']['different_pixels']==0 for r in report),'negative_controls':negative,'original_reader_portable':portable,'scope':'7 actual legend snapshot/shared PDF/native GPU consumers; source font/platform bound'},indent=2)+'\n')
print(out.read_text());sys.exit(0 if all(r[k]['different_pixels']==0 for r in report for k in ['image','pdf'])else 1)
