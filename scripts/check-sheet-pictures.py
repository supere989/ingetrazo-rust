#!/usr/bin/env python3
"""Strict frozen physical picture PNG/JPEG/PDF and original-reader comparison."""
import argparse, copy, hashlib, json, os, sys, tempfile
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
from PySide6.QtCore import QSize
from PySide6.QtGui import QImage,QPainter,QColor
from PySide6.QtPdf import QPdfDocument
from PySide6.QtWidgets import QApplication
PIN='6be29fe437a7cd992d4e079f71720821b25216b9'
NAMES=['picture-import-plain','picture-jpeg-stretch','picture-cover','picture-contain','picture-rounded-border','picture-ellipse-feather-opacity','picture-rounded-feather','picture-opacity-rect','picture-hidden','picture-missing','picture-overlap']
FLAGS=['nonmutating','external_paths','snapshot_assets','warm_images']
def contained(root,name):
 assert type(name)is str and name
 path=root/name;path.resolve().relative_to(root.resolve());return path

def inventory(source,native):
 assert source['source_pin']==PIN
 assert type(source['cases'])is list and type(native['cases'])is list
 assert [r['case']for r in source['cases']]==NAMES
 assert [r['case']for r in native['cases']]==NAMES
 assert native['atomic_refusal']is True and native['duplicate_owner']is True
 for a,b in zip(source['cases'],native['cases']):
  assert type(a['dpi'])is int and type(b['dpi'])is int and b['dpi']==a['dpi']==100
  assert a['source_reopen']is True
  assert all(b[key]is True for key in FLAGS)
  assert set(b['images'])=={'png','jpg'}
  for extension in ['png','jpg']:
   x,y=a['images'][extension],b['images'][extension]
   assert type(y['file'])is str and y['file']==x['file']==a['case']+'.'+extension
   assert type(y['pixels'])is list and len(y['pixels'])==2 and all(type(v)is int and v>0 for v in y['pixels'])
   assert y['pixels']==x['size']
  assert b['pdf']==a['pdf']==a['case']+'.pdf'
  assert b['portable']==a['case']+'.igz'

def rgb(path):
 image=QImage(str(path));assert not image.isNull(),str(path)
 image=image.convertToFormat(QImage.Format_RGB888);data=bytes(image.constBits());w,h=image.width(),image.height()
 return [w,h],b''.join(data[y*image.bytesPerLine():y*image.bytesPerLine()+w*3]for y in range(h))

def difference(a,b):
 sa,aa=rgb(a);sb,bb=rgb(b);assert sa==sb,('pixel dimensions',sa,sb)
 return dict(pixels=sa,different_pixels=sum(aa[i:i+3]!=bb[i:i+3]for i in range(0,len(aa),3)),max_channel_error=max((abs(x-y)for x,y in zip(aa,bb)),default=0),source_rgb_sha256=hashlib.sha256(aa).hexdigest(),native_rgb_sha256=hashlib.sha256(bb).hexdigest())

def pdf_image(path,destination):
 doc=QPdfDocument();assert doc.load(str(path))==QPdfDocument.Error.None_,str(path)
 assert doc.pageCount()==1
 points=doc.pagePointSize(0);render=doc.render(0,QSize(round(points.width()*100/72),round(points.height()*100/72)));assert not render.isNull()
 opaque=QImage(render.size(),QImage.Format_RGB32);opaque.fill(0xffffffff);painter=QPainter(opaque);painter.drawImage(0,0,render);painter.end();assert opaque.save(str(destination))

def equal_typed(a,b,path=''):
 assert type(a)is type(b),(path,'type')
 if type(a)is dict:
  assert a.keys()==b.keys(),(path,'keys')
  for k in a:equal_typed(a[k],b[k],path+'/'+k)
 elif type(a)is list:
  assert len(a)==len(b),(path,'length')
  for i,(x,y)in enumerate(zip(a,b)):equal_typed(x,y,path+'/'+str(i))
 else:assert a==b,(path,a,b)

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('source',type=Path);parser.add_argument('native',type=Path);parser.add_argument('comparison',type=Path);args=parser.parse_args()
 source,native,out=args.source.resolve(),args.native.resolve(),args.comparison.resolve();app=QApplication.instance()or QApplication([])
 report=dict(comparisons=[],negative_controls=[],original_reader_portable=[],result='incomplete',scope='11 strict actual physical picture PNG/JPEG/PDF exports and source reader. picture-overlap contains Linux-bound Sans Serif text; no Mac font tolerance or full platform parity.')
 def persist():out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps(report,indent=2)+'\n')
 try:
  src=json.loads((source/'source.json').read_text());got=json.loads((native/'native.json').read_text());inventory(src,got)
  errors=[]
  with tempfile.TemporaryDirectory()as temp:
   temp=Path(temp)
   for a,b in zip(src['cases'],got['cases']):
    row=dict(case=a['case'],font_scope='Linux source Sans Serif'if a['case']=='picture-overlap'else'no text')
    for extension in ['png','jpg']:
     try:
      x=contained(source,a['images'][extension]['file']);y=contained(native,b['images'][extension]['file']);size,data=rgb(x)
      assert size==a['images'][extension]['size'] and hashlib.sha256(data).hexdigest()==a['images'][extension]['rgb_sha256']
      row[extension]=difference(x,y)
     except Exception as e:row[extension]={'error':str(e)};errors.append(a['case']+'/'+extension)
    try:
     x=temp/(a['case']+'-source.png');y=temp/(a['case']+'-native.png');pdf_image(contained(source,a['pdf']),x);pdf_image(contained(native,b['pdf']),y);row['pdf']=difference(x,y)
    except Exception as e:row['pdf']={'error':str(e)};errors.append(a['case']+'/pdf')
    report['comparisons'].append(row);persist()
  # All pixel differences survive a subsequent reader/control failure.
  sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'ingetrazo'))
  from core.scene import Scene
  from formats import igz
  for a,b in zip(src['cases'],got['cases']):
   left,right=Scene(),Scene();igz.load_into(left,contained(source,a['scene']));igz.load_into(right,contained(native,b['portable']))
   assert len(left.compositions)==len(right.compositions)==1
   l=[vars(v)for v in left.compositions[0].images];r=[vars(v)for v in right.compositions[0].images]
   equal_typed(l,a['source_model'],'source-model');equal_typed(l,r,'native-reader')
   report['original_reader_portable'].append(a['case']);persist()
  for name,mutate in [
   ('missing-native',lambda a,b:b['cases'].pop()),('jointly-empty',lambda a,b:(a.update(cases=[]),b.update(cases=[]))),
   ('jointly-truncated',lambda a,b:(a['cases'].pop(),b['cases'].pop())),('duplicate-case',lambda a,b:b['cases'].__setitem__(-1,b['cases'][0])),
   ('numeric-dpi',lambda a,b:b['cases'][0].update(dpi=100.0)),('boolean-pixel',lambda a,b:b['cases'][0]['images']['png']['pixels'].__setitem__(0,True)),
   ('missing-jpeg',lambda a,b:b['cases'][0]['images'].pop('jpg')),('wrong-file',lambda a,b:b['cases'][0]['images']['png'].update(file='wrong.png')),
   ('numeric-nonmutating',lambda a,b:b['cases'][0].update(nonmutating=1)),('false-external-paths',lambda a,b:b['cases'][0].update(external_paths=False)),
   ('false-snapshot-assets',lambda a,b:b['cases'][0].update(snapshot_assets=False)),('false-warm-images',lambda a,b:b['cases'][0].update(warm_images=False)),
   ('false-atomic',lambda a,b:b.update(atomic_refusal=False)),('false-duplicate-owner',lambda a,b:b.update(duplicate_owner=False))]:
   a,b=copy.deepcopy(src),copy.deepcopy(got);mutate(a,b)
   try:inventory(a,b)
   except(AssertionError,KeyError,TypeError):report['negative_controls'].append(name)
   else:raise AssertionError('Schema corruption accepted: '+name)
  exact=next((r for r in report['comparisons']if r['png'].get('different_pixels')==0),None);assert exact is not None,'No exact baseline exists for independent pixel corruption'
  with tempfile.TemporaryDirectory()as temp:
   temp=Path(temp);im=QImage(str(contained(native,exact['case']+'.png')));old=im.pixelColor(0,0);im.setPixelColor(0,0,QColor((old.red()+1)%256,old.green(),old.blue()));changed=temp/'one.png';assert im.save(str(changed));assert difference(source/(exact['case']+'.png'),changed)['different_pixels']==1;report['negative_controls'].append('one-pixel-on-exact-baseline')
   empty=temp/'empty.pdf';empty.write_bytes(b'')
   try:pdf_image(empty,temp/'empty.png')
   except AssertionError:report['negative_controls'].append('empty-pdf')
   else:raise AssertionError('Empty PDF accepted')
  original=copy.deepcopy(src['cases'][0]['source_model']);mutated=copy.deepcopy(original);assert mutated;mutated[0]['path']='assets/wrong.png'
  try:equal_typed(original,mutated)
  except AssertionError:report['negative_controls'].append('reader-path-loss')
  else:raise AssertionError('Reader path mutation accepted')
  for extension in ['png','jpg','pdf']:report['exact_'+extension]=sum(r[extension].get('different_pixels')==0 for r in report['comparisons'])
  report['result']='pass'if not errors and all(report['exact_'+extension]==11 for extension in ['png','jpg','pdf'])else'fail'
  if errors:report['decode_errors']=errors
 except Exception as e:report['result']='fail';report['error']=str(e)
 finally:persist()
 print(json.dumps(report));return 0 if report['result']=='pass'else 1
if __name__=='__main__':sys.exit(main())
