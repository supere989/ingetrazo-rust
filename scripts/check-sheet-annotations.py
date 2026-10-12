#!/usr/bin/env python3
"""Strict six-family physical PNG/JPEG/PDF and original-reader acceptance."""
import argparse,copy,hashlib,json,math,os,tempfile,zipfile
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QImage,QPainter,QColor
from PySide6.QtPdf import QPdfDocument
from PySide6.QtCore import QSize
NAMES=['cotas-default','cotas_ang-default','cotas_rad-default','etiquetas-default','niveles-default','llamadas-default','cotas-positions','cotas-placement','cotas-units-scale','angular-sweep','radial-fit','labels-leaders','labels-wrapped','levels-symbols','callouts-leaders','field-context','all-six-defaults','mixed-same-z','mixed-high-z','all-six-hidden']
def equal(a,b,p='$'):
 assert type(a)is type(b),(p,'type',type(a).__name__,type(b).__name__)
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(p,'keys',list(a),list(b))
  for k in a:equal(a[k],b[k],p+'.'+k)
 elif isinstance(a,list):
  assert len(a)==len(b),(p,'length',len(a),len(b))
  for i,(x,y)in enumerate(zip(a,b)):equal(x,y,p+'.'+str(i))
 else:
  if isinstance(a,float):assert math.isfinite(a)and math.isfinite(b),(p,'nonfinite')
  assert a==b,(p,a,b)
def inventories(s,n):
 assert s['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [x['case']for x in s['images']]==NAMES and [x['case']for x in n['images']]==NAMES
 assert n['atomic_refusal']is True
 for a,b in zip(s['images'],n['images']):
  assert b['nonmutating']is True and b['snapshot_owner']is True
  assert type(b['dpi'])is int and b['dpi']==a['dpi']
  assert type(b['sheet_index'])is int and b['sheet_index']==a['sheet_index']
  assert b['file']==a['image']['file'] and b['pdf']==a['pdf'] and b['jpeg']==a['jpeg']
  assert b['warm_reused']is True

def rgb(path):
 image=QImage(str(path));assert not image.isNull(),str(path);image=image.convertToFormat(QImage.Format_RGB888);data=bytes(image.constBits());return[image.width(),image.height()],b''.join(data[y*image.bytesPerLine():y*image.bytesPerLine()+image.width()*3]for y in range(image.height()))
def difference(a,b):
 sa,aa=a;sb,bb=b;assert sa==sb,('image-size',sa,sb);return dict(size=sa,different_pixels=sum(aa[i:i+3]!=bb[i:i+3]for i in range(0,len(aa),3)),max_channel_error=max((abs(a-b)for a,b in zip(aa,bb)),default=0),source_rgb_sha256=hashlib.sha256(aa).hexdigest(),native_rgb_sha256=hashlib.sha256(bb).hexdigest())
def pdf(path,dpi):
 doc=QPdfDocument();assert doc.load(str(path))==QPdfDocument.Error.None_,str(path);assert doc.pageCount()==1;points=doc.pagePointSize(0);image=doc.render(0,QSize(round(points.width()*dpi/72),round(points.height()*dpi/72)));assert not image.isNull();opaque=QImage(image.size(),QImage.Format_RGB32);opaque.fill(0xffffffff);p=QPainter(opaque);p.drawImage(0,0,image);p.end();opaque=opaque.convertToFormat(QImage.Format_RGB888);data=bytes(opaque.constBits());pixels=b''.join(data[y*opaque.bytesPerLine():y*opaque.bytesPerLine()+opaque.width()*3]for y in range(opaque.height()));return([opaque.width(),opaque.height()],pixels),[points.width(),points.height()]
def guard(doc,expected):
 for path,value in expected['paths'].items():
  v=doc
  for k in path.split('.'):v=v[int(k)]if isinstance(v,list)else v[k]
  equal(value,v,'raw.'+path)
def safe(folder,name):
 p=(folder/name).resolve();assert p.is_relative_to(folder.resolve()),name;return p
p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);p.add_argument('comparison',type=Path);a=p.parse_args();app=QApplication([]);s=json.loads((a.source/'source.json').read_text());n=json.loads((a.native/'native.json').read_text());report=dict(result='pending',comparisons=[],negative_controls=[],scope='Actual six-family physical sheet PNG/JPEG/PDF and original reader. Linux Qt/font-bound strict pixels; no UI authoring or Model View anchor synchronization proof.');a.comparison.parent.mkdir(parents=True,exist_ok=True)
def write():a.comparison.write_text(json.dumps(report,indent=2)+'\n')
try:
 inventories(s,n)
 for source,native in zip(s['images'],n['images']):
  src=rgb(safe(a.source,source['image']['file']));assert src[0]==source['image']['size']and hashlib.sha256(src[1]).hexdigest()==source['image']['rgb_sha256'];image=difference(src,rgb(safe(a.native,native['file'])));jpg=difference(rgb(safe(a.source,source['jpeg'])),rgb(safe(a.native,native['jpeg'])));sp,ss=pdf(safe(a.source,source['pdf']),source['dpi']);np,ns=pdf(safe(a.native,native['pdf']),source['dpi']);equal(ss,ns,'pdf-page-points');pr=difference(sp,np);report['comparisons'].append(dict(case=source['case'],png=image,jpeg=jpg,pdf=pr,page_points=ss));write()
 sr=json.loads((a.source/'original-reopens.json').read_text());nr=json.loads((a.native/'native-reopens.json').read_text());assert [r['case']for r in sr]==NAMES and [r['case']for r in nr]==NAMES;equal(sr,nr,'original-readers');report['original_reader_pairs']=len(nr)
 expected=json.loads((a.source/'opaque-guards.json').read_text());row=next(x for x in n['images']if x['case']==expected['case']);data=safe(a.native,row['portable']).read_bytes();raw=json.loads(data)if data[:2]!=b'PK'else json.loads(zipfile.ZipFile(safe(a.native,row['portable'])).read('document.json'));guard(raw,expected)
except (AssertionError,KeyError,TypeError,IndexError,ValueError,OSError)as error:
 report.update(result='fail',error=repr(error));write();raise SystemExit(repr(error))
negative=[]
for name,change in [('missing-case',lambda s,n:n['images'].pop()),('jointly-empty',lambda s,n:(s.update(images=[]),n.update(images=[]))),('jointly-truncated',lambda s,n:(s['images'].pop(),n['images'].pop())),('duplicate-case',lambda s,n:n['images'].__setitem__(-1,n['images'][0])),('reordered-cases',lambda s,n:n['images'].reverse()),('bool-dpi',lambda s,n:n['images'][0].update(dpi=True)),('bool-sheet-index',lambda s,n:n['images'][0].update(sheet_index=False)),('false-nonmutating',lambda s,n:n['images'][0].update(nonmutating=False)),('numeric-snapshot',lambda s,n:n['images'][0].update(snapshot_owner=1)),('false-warm',lambda s,n:n['images'][0].update(warm_reused=False)),('false-atomic',lambda s,n:n.update(atomic_refusal=False)),('wrong-png',lambda s,n:n['images'][0].update(file='wrong.png')),('wrong-pdf',lambda s,n:n['images'][0].update(pdf='wrong.pdf')),('missing-jpeg',lambda s,n:n['images'][0].pop('jpeg'))]:
 ss=copy.deepcopy(s);nn=copy.deepcopy(n);change(ss,nn)
 try:inventories(ss,nn)
 except (AssertionError,KeyError,TypeError,IndexError):negative.append(name)
 else:raise AssertionError('Accepted '+name)
for name,change in [('reader-truncated',lambda d:d.pop()),('reader-layered-item-loss',lambda d:d[0]['compositions'][0].pop('cotas')),('reader-bool-width',lambda d:d[0]['compositions'][0]['cotas'][0].update(dx_mm=True))]:
 rr=copy.deepcopy(nr);change(rr)
 try:equal(sr,rr,'readers')
 except (AssertionError,KeyError,TypeError,IndexError):negative.append(name)
 else:raise AssertionError('Accepted '+name)
for name,change in [('opaque-sheet-loss',lambda d:d['scene']['compositions'][0].pop('vendor_annotations')),('opaque-family-type-loss',lambda d:d['scene']['compositions'][0]['cotas_rad'][0]['vendor_family'].update(opaque=[True,'one',None,False]))]:
 r=copy.deepcopy(raw);change(r)
 try:guard(r,expected)
 except (AssertionError,KeyError,TypeError,IndexError):negative.append(name)
 else:raise AssertionError('Accepted '+name)
# Independent exact SOURCE baseline validates a one-pixel rejection even if native pixels differ.
with tempfile.TemporaryDirectory()as t:
 src=safe(a.source,s['images'][0]['image']['file']);im=QImage(str(src));old=im.pixelColor(0,0);im.setPixelColor(0,0,QColor(0,old.green(),old.blue()));path=Path(t)/'one.png';assert im.save(str(path));assert difference(rgb(src),rgb(path))['different_pixels']==1;negative.append('one-pixel-source-baseline')
 empty=Path(t)/'empty.pdf';empty.write_bytes(b'');doc=QPdfDocument();assert doc.load(str(empty))!=QPdfDocument.Error.None_;negative.append('empty-pdf')
report.update(negative_controls=negative,exact_png=sum(x['png']['different_pixels']==0 for x in report['comparisons']),exact_jpeg=sum(x['jpeg']['different_pixels']==0 for x in report['comparisons']),exact_pdf=sum(x['pdf']['different_pixels']==0 for x in report['comparisons']));report['result']='pass'if all(x[k]['different_pixels']==0 for x in report['comparisons']for k in ['png','jpeg','pdf'])else'fail';write();print(json.dumps(report));raise SystemExit(0 if report['result']=='pass'else 1)
