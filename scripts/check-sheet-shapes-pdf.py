#!/usr/bin/env python3
"""Independent QtPdf/PDFium whole-page comparison; strict pixels."""
import os,sys,json
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM","offscreen")
from PySide6.QtWidgets import QApplication
from PySide6.QtPdf import QPdfDocument
from PySide6.QtGui import QImage,QPainter
from PySide6.QtCore import QSize
app=QApplication([]);out=Path(sys.argv[1]);dpi=int(sys.argv[2]) if len(sys.argv)>2 else 72;assert dpi in (72,144,300);docs=[]
for name in ("original","native"):
 doc=QPdfDocument();assert doc.load(str(out/(name+".pdf")))==QPdfDocument.Error.None_;assert doc.pageCount()==9;docs.append(doc)
rows=[]
for i in range(9):
 a,b=docs;assert a.pagePointSize(i)==b.pagePointSize(i)
 images=[]
 for name,doc in zip(("original","native"),docs):
  raw=doc.render(i,QSize(round(595*dpi/72),round(842*dpi/72)));assert not raw.isNull()
  image=QImage(raw.size(),QImage.Format.Format_RGB32);image.fill(0xffffffff)
  p=QPainter(image);p.drawImage(0,0,raw);p.end();image=image.convertToFormat(QImage.Format.Format_RGBA8888)
  assert image.save(str(out/(name+"-"+str(i)+".png")));images.append(bytes(image.constBits()))
 aa,bb=images;changed=sum(aa[j:j+4]!=bb[j:j+4] for j in range(0,len(aa),4))
 rows.append({"page":i,"different_pixels":changed,"passed":changed==0})
report={"reader":"QtPdf/PDFium","pages":9,"point_size":[595,842],"dpi":dpi,"passed":all(r["passed"] for r in rows),"cases":rows,"scope":"Actual original atlas vs direct native vector PDF at whole pages, strictRGBA. UI/other painters remain open."}
(out/"result.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report));sys.exit(0 if report["passed"] else 1)
