#!/usr/bin/env python3
"""Six required native macOS PDFs; retain partial diagnostics without accepting refusals."""
import os,sys,json
from pathlib import Path
os.environ.setdefault('QT_QPA_PLATFORM','offscreen')
from PySide6.QtWidgets import QApplication
from PySide6.QtPdf import QPdfDocument
from PySide6.QtGui import QImage,QPainter,QColor
from PySide6.QtCore import QSize
NAMES=['arabic','cjk','helvetica','helvetica-bold','helvetica-italic','times-bold-italic']
def compare_images(source,native):
 if source.size()!=native.size():return {'passed':False,'error':'image dimensions differ'}
 source=source.convertToFormat(QImage.Format.Format_RGBA8888);native=native.convertToFormat(QImage.Format.Format_RGBA8888)
 a,b=bytes(source.constBits()),bytes(native.constBits())
 source_ink=sum(a[i:i+3]!=b'\xff\xff\xff'for i in range(0,len(a),4));native_ink=sum(b[i:i+3]!=b'\xff\xff\xff'for i in range(0,len(b),4))
 changed=sum(a[i:i+4]!=b[i:i+4]for i in range(0,len(a),4))
 return {'passed':source_ink>0 and native_ink>0 and changed==0,'changed_pixels':changed,'source_ink':source_ink,'native_ink':native_ink}
def read_page(path,dpi):
 doc=QPdfDocument()
 if doc.load(str(path))!=QPdfDocument.Error.None_:raise ValueError('actual PDF reader rejected '+str(path))
 if doc.pageCount()!=1:raise ValueError('one page required')
 size=doc.pagePointSize(0);raw=doc.render(0,QSize(round(size.width()*dpi/72),round(size.height()*dpi/72)))
 if raw.isNull():raise ValueError('null PDF rendering')
 image=QImage(raw.size(),QImage.Format.Format_RGB32);image.fill(0xffffffff);p=QPainter(image);p.drawImage(0,0,raw);p.end()
 return image,[size.width(),size.height()]
def main():
 app=QApplication([]);original=Path(sys.argv[1]);out=Path(sys.argv[2]);dpi=int(sys.argv[3]);assert dpi in [72,150,300]
 source=json.loads((original/'original-macos-fonts.json').read_text());assert source['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9' and source['qt']=='6.12.0'
 assert sorted(row['name']for row in source['cases'])==NAMES
 native=json.loads((out/'native-pdf.json').read_text());assert sorted(row['name']for row in native)==NAMES
 by_name={row['name']:row for row in native};assert len(by_name)==6
 extras=sorted(p.name for p in out.glob('*.native.pdf') if p.name.removesuffix('.native.pdf')not in NAMES);assert not extras
 rows=[];controls=[]
 for name in NAMES:
  record=by_name[name];row={'case':name,'passed':False}
  try:
   src,size=read_page(original/(name+'.original.pdf'),dpi)
   # Both negatives pass through the same pixel admission/comparison as positives.
   blank=QImage(src.size(),src.format());blank.fill(0xffffffff);blank_result=compare_images(src,blank);assert not blank_result['passed']
   altered=src.copy();pixel=altered.pixelColor(0,0);altered.setPixelColor(0,0,QColor(0 if pixel.red()!=0 else 255,pixel.green(),pixel.blue()));changed=compare_images(src,altered);assert not changed['passed'] and changed['changed_pixels']==1
   controls.append({'case':name,'blank_rejected':True,'one_pixel_mutation_rejected':True,'mutation_changed_pixels':1})
   if record.get('passed')is not True:row['error']='native refusal: '+str(record.get('error','missing successful record'))
   elif not record.get('warm_layout_unchanged')or not record.get('warm_pdf_bytes_exact'):row['error']='missing warm resource proof'
   elif not (out/(name+'.native.pdf')).is_file():row['error']='native PDF missing'
   else:
    got,native_size=read_page(out/(name+'.native.pdf'),dpi)
    row.update(compare_images(src,got));row['source_page_points']=size;row['native_page_points']=native_size
    if size!=native_size:row['passed']=False;row['error']='page dimensions differ'
    src.save(str(out/(name+'.original.'+str(dpi)+'.png')));got.save(str(out/(name+'.native.'+str(dpi)+'.png')))
  except Exception as error:row['error']=repr(error)
  rows.append(row)
 report={'passed':len(rows)==6 and all(r['passed']for r in rows) and len(controls)==6,'dpi':dpi,'cases':rows,'negative_controls':controls,'scope':'Actual required six native CoreText production painter/direct PDF pages vs frozen original; partial diagnostics do not accept missing/refused cases; CanvasGPU/UI separate'}
 (out/('reader-'+str(dpi)+'.json')).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));return 0 if report['passed']else 1
if __name__=='__main__':sys.exit(main())
